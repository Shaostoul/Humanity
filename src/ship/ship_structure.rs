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
//! each row names two zones and extrudes a straight, axis-aligned box tube between their facing
//! perimeter planes (floor slab + two side walls + a glass-or-shell lid), cuts a door-sized
//! aperture through each zone's perimeter shell where the tube meets it (mesh AND collision, so
//! the hallway is genuinely walkable, not decoration), and registers a walkable room bound per
//! corridor. The corridor OWNS its door mouths (`lat` + `door_width`/`door_height` on the row);
//! it deliberately does NOT reference authored doors. The first cut of increment B indexed each
//! zone's door list by ordinal, and the operator hit both failure modes: moving/adding/removing
//! ANY door silently retargeted every corridor (positional indices into a filtered,
//! order-dependent list), and the authored door wall sat coplanar with the generated perimeter
//! shell at the mouth (two walls where one belongs: z-fighting + a walk-through-wall seam).
//! Increment C (the Commons authoring) is pure data on top: a big glass-roofed zone + machines +
//! corridor rows -- no schema change needed here.
//!
//! Increment 1a of docs/design/ship-homes-and-logistics.md (2026-10-03): THE SHIP AND THE HOME
//! COME APART. Two files now, one frame (ship metres, y up, the origin at plot p1's corner):
//! - `data/blueprints/ship_structure.ron` (the SHIP FILE): the shared zones (the Commons, the
//!   first street), the corridors between them, the `plots` homes go on, the `default_plot`
//!   offline play uses, and the mothership's macro `districts`. No home is in it.
//! - `data/homes/<kind>.ron` (a HOME DESIGN): one home's body plus the point on its shell where
//!   a plot's corridor arrives. It carries no position.
//!
//! `ShipStructure::assemble` builds the runtime ship every other module already reads: the ship
//! file, plus the player's home as zone id `home` at their plot's origin, plus that plot's door
//! corridor. So `home` is a LOCAL ALIAS for the viewer's own home, and every caller that looks
//! the home up by that id (the machines' default zone, `PLAYER_HOME`, the editor) keeps working.
//! Saving splits it again (`save_assembled`): the home design is written in every play mode,
//! the ship file only with ShipStructureEditing, and the home's origin never, because it comes
//! from the plot. `validate` refuses overlapping plots, zones and corridor tubes.

use crate::renderer::mesh::Vertex;
use crate::ship::fibonacci::{floor_quad, wall_box, HomesteadMeshes, RoomInfo};
use crate::ship::home_structure::{HomeStructure, ShellCut, Zone};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The zone id the assembled home always carries: a local alias for "the viewer's own home".
pub const HOME_ZONE_ID: &str = "home";
/// The ship file, relative to the data dir (also its key in `embedded_data`).
pub const SHIP_FILE: &str = "blueprints/ship_structure.ron";
/// The folder home designs live in, relative to the data dir (`homes/<kind>.ron`).
pub const HOME_DESIGNS_DIR: &str = "homes";
/// How far two boxes may poke into each other and still count as touching, not overlapping.
const OVERLAP_EPS: f32 = 1e-3;

fn default_purpose() -> String {
    "residence".to_string()
}

fn default_corridor_width() -> f32 {
    3.0
}

fn default_corridor_door_width() -> f32 {
    2.0
}

fn default_corridor_door_height() -> f32 {
    2.2
}

/// Corridor side-wall thickness (metres) -- matches the legacy interior-wall default (0.15 m).
pub const CORRIDOR_WALL_THICKNESS: f32 = 0.15;
/// Tiny vertical clearance so a tube's floor/lid never sit COPLANAR with a zone's floor/ceiling
/// where the tube overlaps the zone footprint (coplanar quads z-fight). 1 cm: imperceptible, and
/// collision is a 2D XZ push-out so it changes nothing gameplay-side.
const CORRIDOR_SURFACE_EPS: f32 = 0.01;
/// Minimum run length (metres): the clear gap between the two zone boxes must be at least this,
/// or the "corridor" is really a doorway between touching (or overlapping) boxes.
const CORRIDOR_MIN_RUN: f32 = 0.25;
/// Minimum corridor width (metres): narrower than this is not walkable (player radius 0.3).
const CORRIDOR_MIN_WIDTH: f32 = 0.5;
/// Minimum door-mouth height (metres): lower than this reads as a crawl vent, not a doorway.
const CORRIDOR_MIN_DOOR_HEIGHT: f32 = 1.0;

/// One generated corridor (ship-superstructure increment B, reworked): a straight, axis-aligned
/// tube between two zones. The corridor OWNS its door mouths instead of referencing authored
/// doors: `lat` places the tube centreline in WORLD coordinates on the axis across the run, and
/// `door_width`/`door_height` size the aperture it cuts through EACH zone's perimeter shell. The
/// run axis is not stored -- it derives from the clear gap between the two zone boxes, so dragging
/// a zone origin re-resolves honestly instead of desyncing. (The original schema indexed each
/// zone's door list by ordinal; the operator hit both consequences: any door edit silently
/// retargeted every corridor, and the authored door wall z-fought the generated shell at the
/// mouth.) v1 corridors are STRAIGHT: validation rejects zone pairs with no clear axis gap
/// (L-bends are a documented follow-up: two segments + an elbow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipCorridor {
    pub from_zone: String,
    pub to_zone: String,
    /// The tube centreline's lateral position in WORLD coordinates: world z for an X-run
    /// corridor, world x for a Z-run. Validation requires the whole door mouth
    /// (`lat` +/- `door_width`/2) to land inside BOTH zones' spans on that axis.
    pub lat: f32,
    /// Tube width in metres (outer, across the run). Side walls sit AT the edges, so the clear
    /// interior is width minus one wall thickness.
    #[serde(default = "default_corridor_width")]
    pub width: f32,
    /// Width of the door mouth this corridor cuts through each zone's perimeter shell.
    #[serde(default = "default_corridor_door_width")]
    pub door_width: f32,
    /// Height of the door mouth (clamped to the tube height at resolve time).
    #[serde(default = "default_corridor_door_height")]
    pub door_height: f32,
    /// Glass lid (rides the transparent always-visible ceiling pass, exactly like a glass zone
    /// roof); false = an opaque lid in the show-roof-gated opaque pass, like a steel zone roof.
    #[serde(default)]
    pub glass_top: bool,
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
    /// World span along the run axis (start < end): the two zones' FACING perimeter planes.
    pub start: f32,
    pub end: f32,
    /// World lateral centreline (z when axis = X; x when axis = Z): the row's own `lat`,
    /// validated to keep the whole door mouth inside both zones' spans on that axis.
    pub lat: f32,
    /// World floor (deck) height: the zones' shared origin y (v1 corridors are level).
    pub floor_y: f32,
    /// Interior tube height: the SHORTER end's ground storey (a zone's first storey, a plot's box
    /// height), so the lid never rises above either roofline or past a ground floor's ceiling, and
    /// the door mouth (clamped to this) always fits.
    pub height: f32,
    pub width: f32,
    pub glass_top: bool,
    /// The two mouth centres at floor level, one on each zone's facing perimeter plane -- the
    /// tube's endpoints.
    pub end_from: Vec3,
    pub end_to: Vec3,
    pub from_zone_idx: usize,
    pub to_zone_idx: usize,
    /// The from/to door apertures' (width, height) -- what the shell cuts open. Both mouths are
    /// the corridor's own door size now (kept as a pair so consumers stay shape-stable).
    pub door_from: (f32, f32),
    pub door_to: (f32, f32),
}

/// One corridor DOOR MOUTH resolved to world space (corridor door panels, v0.795): a perimeter
/// plane the tube pierces, where a pair of sliding door panels lives. Every valid corridor has two
/// end mouths (one on each zone's facing shell); an INTERVENING zone whose perimeter crosses the
/// run contributes a mouth per crossing plane -- the SAME planes `shell_cuts_for_zone` opens
/// apertures through. The two must stay in lockstep: a mouth without a cut puts a door panel
/// inside solid wall, a cut without a mouth leaves an aperture permanently doorless.
#[derive(Debug, Clone, PartialEq)]
pub struct CorridorMouth {
    pub axis: CorridorAxis,
    /// World coordinate of the mouth plane ALONG the run axis (x for an X-run, z for a Z-run).
    pub plane: f32,
    /// World centre of the aperture ACROSS the run (z for an X-run, x for a Z-run): the row's lat.
    pub lat: f32,
    /// World deck height -- the door panels' sill (v1 corridors are level, so both end zones and
    /// the tube share this y).
    pub floor_y: f32,
    /// Aperture (width, height) at this plane -- the corridor's own door mouth, with the height
    /// re-clamped to an intervening zone's box height exactly like its shell cut is.
    pub door: (f32, f32),
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

/// The door of a plot: the shared zone its corridor runs to, and that corridor's own lat and
/// mouth (the same fields a `ShipCorridor` row carries; the plot's home is the other end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotDoor {
    /// The shared zone (a street, the Commons) the plot's corridor runs to.
    pub zone: String,
    /// The corridor's centreline in SHIP metres across the run (z for an X run, x for a Z run).
    /// It must meet the home design's door point (`HomeDesign::door_lat`), which assembly checks.
    pub lat: f32,
    #[serde(default = "default_corridor_width")]
    pub width: f32,
    #[serde(default = "default_corridor_door_width")]
    pub door_width: f32,
    #[serde(default = "default_corridor_door_height")]
    pub door_height: f32,
    #[serde(default)]
    pub glass_top: bool,
}

impl PlotDoor {
    /// The corridor row this door makes when the plot's home is the zone `from_zone`.
    pub fn corridor_from(&self, from_zone: &str) -> ShipCorridor {
        ShipCorridor {
            from_zone: from_zone.to_string(),
            to_zone: self.zone.clone(),
            lat: self.lat,
            width: self.width,
            door_width: self.door_width,
            door_height: self.door_height,
            glass_top: self.glass_top,
        }
    }
}

/// A PLOT (docs/design/ship-homes-and-logistics.md section 2.3): where one household's home
/// goes on the ship. Geometry lives here in data, never in the database; a relay only records
/// who holds which plot (increment 1b).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plot {
    /// Stable id ("p1"), never a list index.
    pub id: String,
    /// The block plot this one sits inside (Cabins and Apartments, later); None for a Homestead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Which home designs fit: the design file `data/homes/<kind>.ron`.
    pub kind: String,
    /// The plot box's min corner in ship metres (y = deck height).
    pub origin: (f32, f32, f32),
    /// The plot box: (width X, height Y, depth Z) metres. The home's body must fit inside it.
    pub size: (f32, f32, f32),
    /// Where the plot's corridor goes.
    pub door: PlotDoor,
    /// Which neighbourhood it belongs to (none are defined yet; empty).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub neighbourhood: String,
}

impl Plot {
    /// The plot box (min, max) in ship metres.
    pub fn aabb(&self) -> (Vec3, Vec3) {
        let o = Vec3::new(self.origin.0, self.origin.1, self.origin.2);
        (o, o + Vec3::new(self.size.0, self.size.1, self.size.2))
    }

    fn end_box(&self) -> EndBox {
        EndBox {
            origin: Vec3::new(self.origin.0, self.origin.1, self.origin.2),
            w: self.size.0,
            d: self.size.2,
            h: self.size.1,
            deck_h: self.size.1,
        }
    }
}

/// A HOME DESIGN file (`data/homes/<kind>.ron`): one home's body, and the point on its shell
/// where a plot's door corridor arrives. No position: assembly puts it at a plot's origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeDesign {
    /// Which plots it fits (a plot's `kind`); also its file name.
    pub kind: String,
    /// The door point (x, z) in home-local metres, on the shell (an east/west wall for a door
    /// an X-run corridor meets, a north/south wall for a Z run).
    pub door: (f32, f32),
    /// The home itself, in home-local metres. Its `spawn` is where the player arrives.
    pub body: HomeStructure,
}

impl HomeDesign {
    /// The corridor lat (ship metres) a plot at `origin` needs to meet this design's door, or
    /// why the door point is not on the shell.
    pub fn door_lat(&self, origin: (f32, f32, f32)) -> Result<f32, String> {
        let (x, z) = self.door;
        let (w, d) = (self.body.width, self.body.depth);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        if near(x, 0.0) || near(x, w) {
            Ok(origin.2 + z) // an east or west wall: the corridor runs along X, lat is a z
        } else if near(z, 0.0) || near(z, d) {
            Ok(origin.0 + x) // a north or south wall: the corridor runs along Z, lat is an x
        } else {
            Err(format!(
                "the {} design's door point ({x}, {z}) is not on its {w} x {d} m shell",
                self.kind
            ))
        }
    }

    /// Load `data/homes/<kind>.ron`: disk first, else the copy built into the exe (a fresh
    /// install or a throwaway relay). A file that does not parse is moved aside, never
    /// overwritten, exactly like the ship file.
    pub fn load(data_dir: &Path, kind: &str) -> Result<HomeDesign, String> {
        if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("'{kind}' is not a home kind (letters, digits, - and _ only)"));
        }
        let rel = format!("{HOME_DESIGNS_DIR}/{kind}.ron");
        let path = data_dir.join(&rel);
        let (text, on_disk) = match std::fs::read_to_string(&path) {
            Ok(t) => (t, true),
            Err(e) => match crate::embedded_data::get_embedded(&rel) {
                Some(t) => {
                    crate::embedded_data::note_builtin_copy(&rel, format_args!("{} could not be read ({e})", path.display()));
                    (t.to_string(), false)
                }
                None => return Err(format!("no home design {} on disk or built in", path.display())),
            },
        };
        match ron::from_str::<HomeDesign>(&text) {
            Ok(d) if d.kind == kind => Ok(d),
            Ok(d) => Err(format!("{} says kind '{}', not '{kind}'", path.display(), d.kind)),
            Err(e) => {
                if on_disk {
                    quarantine(&path, &format!("failed to parse: {e}"));
                }
                Err(format!("{} failed to parse: {e}", path.display()))
            }
        }
    }

    /// The SHIPPED design of `kind`: the copy built into the exe, never the file on disk, which
    /// is this player's own home and may be edited. What a NEIGHBOUR's plot of that kind is drawn
    /// as (increment 2, src/ship/neighbours.rs): every other player's home looks like the
    /// default until homes are shared. None when no design of that kind is built in.
    pub fn built_in(kind: &str) -> Option<HomeDesign> {
        Self::built_in_ref(kind).cloned()
    }

    /// `built_in` without the copy: every shipped design is parsed ONCE per process, the first
    /// time any is asked for, and kept (the embedded text never changes while the game runs).
    /// Increment 2 review, finding 6: the neighbours parsed about 1650 lines of RON twice on every
    /// home rebuild, every frame of an editor drag. A design that does not parse, or names
    /// another kind, is left out (None), as before.
    pub fn built_in_ref(kind: &str) -> Option<&'static HomeDesign> {
        static PARSED: std::sync::OnceLock<std::collections::HashMap<&'static str, HomeDesign>> = std::sync::OnceLock::new();
        PARSED
            .get_or_init(|| {
                crate::embedded_data::SHIPPED_HOMES
                    .iter()
                    .filter_map(|(k, text)| ron::from_str::<HomeDesign>(text).ok().filter(|d| d.kind == *k).map(|d| (*k, d)))
                    .collect()
            })
            .get(kind)
    }

    /// Write back to RON, keeping an existing file's leading comment header (the same
    /// discipline as `ShipStructure::save`). Creates `data/homes/` when it is missing.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let config = ron::ser::PrettyConfig::default().struct_names(false);
        let body = ron::ser::to_string_pretty(self, config).map_err(|e| e.to_string())?;
        let header = preserved_header(path).unwrap_or_else(|| {
            "// HumanityOS home design: one home's body (home-local metres) plus the point on its\n\
             // shell where a plot's door corridor arrives. The loader places it at the player's\n\
             // plot (data/blueprints/ship_structure.ron). docs/design/ship-homes-and-logistics.md.\n\n"
                .to_string()
        });
        std::fs::write(path, format!("{header}{body}")).map_err(|e| e.to_string())
    }
}

/// Which plot and design the `home` zone was assembled from. Never saved: it is rebuilt on
/// every load, and it is what `save_assembled` uses to split the ship back into its two files.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeAssembly {
    /// The plot the home stands on; empty while the home is put away (`away`).
    pub plot: String,
    pub kind: String,
    pub door: (f32, f32),
    /// The home is PUT AWAY (increment 2, `put_home_away`): its player is a guest on this ship,
    /// with no plot, so their home is on no plot of it. The home zone then stands at
    /// `HOME_AWAY_ORIGIN`, off the ship's plan, and nothing of it is drawn, walked into, lit or
    /// wrapped by the hull; every plot is drawn as a neighbour's.
    pub away: bool,
}

/// Where a guest's home is kept while it is put away (`ShipStructure::put_home_away`), ship
/// metres: a kilometre off the ship's plan and 200 m below its deck, a footprint nothing of the
/// ship shares (the carry that moves what a home holds tests x and z only, home_plot.rs
/// `over_plot`), and out of every view from aboard. The home and what it holds (its machines,
/// animals, plants, built pieces and parked vehicles) are moved there with the same carry a
/// relay's welcome uses to move a home between plots, so nothing of it is lost or reset, and
/// they come back the same way. 1.02 km from the origin, inside the 2 km where f32 still
/// resolves a tenth of a millimetre (section 2.2 of the design).
pub const HOME_AWAY_ORIGIN: (f32, f32, f32) = (-1000.0, -200.0, 0.0);

/// A box a corridor can end at: a zone, or a plot standing in for the home it will hold.
#[derive(Debug, Clone, Copy)]
struct EndBox {
    origin: Vec3,
    w: f32,
    d: f32,
    h: f32,
    /// How tall a corridor arriving here may be: the box's GROUND storey (ship homes increment 7).
    /// A corridor meets a zone at its deck, so a two-storey homestead (6.0 m) still caps its
    /// corridor at its ground floor's 3.0 m, not at its roof. A plot, which does not know the
    /// storeys of the home it will hold, offers its whole box height.
    deck_h: f32,
}

impl EndBox {
    fn of_zone(z: &ShipZone) -> EndBox {
        EndBox::of_body(z.origin_vec(), &z.body)
    }

    /// The box a home or zone body makes standing at `origin`, its corridors capped at its ground
    /// storey (`deck_h`).
    fn of_body(origin: Vec3, body: &HomeStructure) -> EndBox {
        let deck_h = body.storeys().first().map_or(body.height, |s| s.top);
        EndBox { origin, w: body.width, d: body.depth, h: body.height, deck_h }
    }

    fn aabb(&self) -> (Vec3, Vec3) {
        (self.origin, self.origin + Vec3::new(self.w, self.h, self.d))
    }
}

/// True when two boxes share volume (touching faces do not count).
fn boxes_overlap(a: &(Vec3, Vec3), b: &(Vec3, Vec3)) -> bool {
    (0..3).all(|k| a.0[k] < b.1[k] - OVERLAP_EPS && b.0[k] < a.1[k] - OVERLAP_EPS)
}

/// True when `inner` lies inside `outer` (faces may coincide).
fn box_contains(outer: &(Vec3, Vec3), inner: &(Vec3, Vec3)) -> bool {
    (0..3).all(|k| inner.0[k] >= outer.0[k] - OVERLAP_EPS && inner.1[k] <= outer.1[k] + OVERLAP_EPS)
}

/// The box a resolved corridor tube fills (floor to lid, wall to wall, mouth to mouth).
fn tube_aabb(g: &CorridorGeom) -> (Vec3, Vec3) {
    let hw = g.width * 0.5;
    match g.axis {
        CorridorAxis::X => (
            Vec3::new(g.start, g.floor_y, g.lat - hw),
            Vec3::new(g.end, g.floor_y + g.height, g.lat + hw),
        ),
        CorridorAxis::Z => (
            Vec3::new(g.lat - hw, g.floor_y, g.start),
            Vec3::new(g.lat + hw, g.floor_y + g.height, g.end),
        ),
    }
}

/// The leading comment block of an existing file, to keep across an editor save (the v0.526
/// lesson: a save must never strip the authored design notes).
fn preserved_header(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().and_then(|existing| {
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
    })
}

/// Move an unloadable file aside (`<name>.invalid-<unix>.ron`) so the fallback can never
/// overwrite the player's data, and the file stays on disk for hand recovery. Best-effort: a
/// failed rename leaves the file in place (still safe: saves only write what actually loaded).
fn quarantine(path: &Path, why: &str) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = path.with_extension(format!("invalid-{stamp}.ron"));
    match std::fs::rename(path, &dest) {
        Ok(()) => log::error!(
            "{} {why}; QUARANTINED to {} (your build data is preserved there; the shipped default loads instead)",
            path.display(),
            dest.display()
        ),
        Err(re) => log::error!(
            "{} {why}; quarantine rename also failed ({re}) -- file left in place",
            path.display()
        ),
    }
}

/// The whole ship: a list of zones. Always at least one (validation enforces it).
///
/// Field order is the file's order on an editor save, so keep `zones` first.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShipStructure {
    pub zones: Vec<ShipZone>,
    /// Generated corridors between zones (increment B; each row owns its door mouths -- see
    /// `ShipCorridor`). Serde-defaulted so a ship with no corridors needs no empty list.
    #[serde(default)]
    pub corridors: Vec<ShipCorridor>,
    /// Where homes go (increment 1a). The ship file lists them; the assembled ship keeps them
    /// so the editor and the build bound know whose box is whose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plots: Vec<Plot>,
    /// The plot offline play uses (the first plot when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_plot: Option<String>,
    /// The mothership's macro districts (residential, hangar, the Concourse, ...) in SHIP metres:
    /// labelled volumes drawn with their zone_filler.ron contents. Until increment 1a they were
    /// sub-zones of the player's home body. Areas, not boxes: overlap rules do not apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub districts: Vec<Zone>,
    /// The ship's name for a relay (increment 1b): the world its plots belong to, so a relay
    /// records "who holds p2 on this ship", and a later fleet keeps one record per ship.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Which plot and design the `home` zone came from (None for a ship file on its own).
    #[serde(skip)]
    pub home: Option<HomeAssembly>,
    /// Plots this game may walk into besides its own, and the one it lights (ship homes 5b,
    /// engine/household.rs): never saved, never in the hash, and not in the neighbours' cache key,
    /// so opening one rebakes nothing.
    #[serde(skip)]
    pub open: OpenPlots,
}

/// The neighbours' plots this game may walk into (ship homes 5b): those it holds a household
/// permit for that the server says works this session, and one it still stands in after its
/// permit stopped working, so nobody is walled in. The game's own business: no other player is
/// told, and it is never written to a file (`ShipStructure::open` is `serde(skip)`). The
/// derives exist only so the field can sit on a serialisable struct.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPlots {
    /// The plots by their ship-file id (`p3`), each walkable: its street or Commons wall open at
    /// its corridor, its corridor and home walls solid to walk against, its doors opening for
    /// this player.
    pub plots: std::collections::BTreeSet<String>,
    /// The one open plot whose shipped design's lights are on: the one this player stands on or
    /// in the corridor of. Charged to nobody's meter.
    pub lit: Option<String>,
}

/// Eye height over the floor, metres: where a player's camera stands, and so where a spawn
/// point puts it (the same 1.7 m the walking camera and `surface_walk::EYE_HEIGHT_M` use).
pub const SPAWN_EYE_HEIGHT_M: f32 = 1.7;

/// A local (x, z) spawn point in a box whose min corner is `origin`, as a ship position at eye
/// height. The one formula for "where a person arrives", so the relay's spawn and the game's
/// camera cannot drift (increment 1b).
fn spawn_at(origin: (f32, f32, f32), local: (f32, f32)) -> Vec3 {
    Vec3::new(origin.0 + local.0, origin.1 + SPAWN_EYE_HEIGHT_M, origin.2 + local.1)
}

/// The middle of a plot of `size` (width, height, depth), as plot-local x and z metres: where
/// someone arrives on a plot when no door is named. ONE formula for the relay
/// (`PlotArrival::arrival` with no door) and the game (`ShipStructure::plot_spawn` for a home
/// design with no authored spawn), each applied to the plot actually handed out, so the two
/// agree on plots of any size (the second review of 1b: the game used to send the middle of the
/// plot it was built on, which the relay then applied to another plot).
fn plot_middle(size: (f32, f32, f32)) -> (f32, f32) {
    (size.0 * 0.5, size.2 * 0.5)
}

/// The one sentence a player reads when a server's ship is not theirs (the design: "one plain
/// sentence: positions only agree when everyone has the same ship"). The relay refuses such a
/// join with it (`game_join_denied`, reason "other_ship") and the game shows it, and keeps
/// showing it under the HUD while it holds (the third review of 1b: a 12 s notice that named
/// no remedy). It says what to do about it.
pub const OTHER_SHIP_SENTENCE: &str =
    "Not joining the shared world: this server has a different ship from yours, and positions only agree when everyone has the same ship, so update whichever of the app and the server is older and reconnect.";

/// The sentence a player reads when the server they joined has no ship at all: its ship file
/// did not load and neither did the copy built into it (`game_join_denied`, reason
/// "no_ship"). The third review of 1b found such a server told every game "a different ship
/// from yours", which sent the player after an update that could not help.
pub const NO_SHIP_SENTENCE: &str =
    "Not joining the shared world: this server's ship did not load, so there is nowhere aboard to stand; its operator can see why in the server's log, and reconnecting after a fix joins it.";

/// The sentence a player reads when their OWN ship did not load: the game then draws the legacy
/// layout, with nowhere aboard to stand and no ship to name in a join. The game shows it (it
/// never joins without a ship, engine/home_plot.rs `join_step`), and a relay refuses a join
/// naming an empty ship with it (`game_join_denied`, reason "no_ship_named"). Round 4 of the
/// 1b review: both cases used to read "this server has a different ship from yours".
pub const OWN_SHIP_SENTENCE: &str =
    "Not joining the shared world: your own ship did not load, so there is nowhere aboard for you to stand; restart the app, and if it happens again the reason is in logs/run.log.";

/// The sentence a player reads when their account on the server was erased while they stood
/// in its shared world (`game_join_denied`, reason "account_erased", sent privately by the
/// relay's erase, relay/handlers/home_plots.rs `leave_world_for_erase`). Round 5 of the 1b
/// review: the erase took the figure out, and nothing told the erasing game, which went on
/// showing the shared world while the relay dropped every update it sent. BUG-135: it named the
/// way back "reconnect", which no control is called; the app now leaves that server on the
/// erase, and the Chat page's Connect is how a person comes back, signing up again.
pub const ERASED_SENTENCE: &str =
    "Out of the shared world: your account on this server was erased, so your figure and your plot there are gone; to come back, open Chat and press Connect, which signs you up again as a new account, with a free plot or a guest place when the ship is full.";

/// One plot as a relay hands it out (increment 1b): the record's id, kind and box. Where its
/// holder arrives depends on their own home's door, which their game names in `game_join`
/// (`arrival`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotArrival {
    pub id: String,
    pub kind: String,
    pub origin: (f32, f32, f32),
    pub size: (f32, f32, f32),
}

/// What a relay needs to hand out plots (increment 1b of docs/design/ship-homes-and-logistics.md):
/// the ship's id and hash, every plot in the ship file's order, and where a guest arrives when
/// every plot is held. Loaded from the same ship file the game assembles from (disk first, else
/// the copy built into the exe, so a throwaway relay with no data folder hands out the same
/// plots).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShipPlots {
    pub ship_id: String,
    pub ship_hash: String,
    pub plots: Vec<PlotArrival>,
    pub guest_spawn: Option<Vec3>,
}

impl ShipPlots {
    /// The ship's plots from `data_dir`: the ship file on disk, else the copy built into the
    /// exe. A file on disk that does not load (moved aside by `ShipStructure::load`, which
    /// logs "the shipped default loads instead") gives way to the built-in copy here too, so
    /// a relay keeps the ship the same version of the game draws (the third review of 1b: it
    /// used to keep NO ship, and every game's join was then refused as "a different ship").
    /// Err only when neither loads.
    pub fn load(data_dir: &Path) -> Result<ShipPlots, String> {
        ShipStructure::ship_for_relay(data_dir).map(|ship| Self::of_ship(&ship))
    }

    /// The plots of a ship file already loaded (what `load` reads; a test builds one by hand).
    pub fn of_ship(ship: &ShipStructure) -> ShipPlots {
        let plots = ship
            .plots
            .iter()
            .map(|p| PlotArrival { id: p.id.clone(), kind: p.kind.clone(), origin: p.origin, size: p.size })
            .collect();
        ShipPlots { ship_id: ship.id.clone(), ship_hash: ship.ship_hash(), plots, guest_spawn: ship.guest_spawn() }
    }

    /// A plot by id.
    pub fn plot(&self, id: &str) -> Option<&PlotArrival> {
        self.plots.iter().find(|p| p.id == id)
    }
}

impl PlotArrival {
    /// Where the holder arrives, at eye height. `local` is their own home's door as their game
    /// names it in `game_join` (plot-local x and z metres, `ShipStructure::home_arrival_local`):
    /// that point on this plot, kept inside the plot's box. No door named (a home with no
    /// authored door, or a scripted player that draws no home), or one that is not a pair of
    /// finite numbers: the middle of THIS plot, which is where the game's own `plot_spawn` puts
    /// a home with no door on it.
    pub fn arrival(&self, local: Option<(f32, f32)>) -> Vec3 {
        match local {
            Some((x, z)) if x.is_finite() && z.is_finite() => {
                spawn_at(self.origin, (x.clamp(0.0, self.size.0), z.clamp(0.0, self.size.2)))
            }
            _ => spawn_at(self.origin, plot_middle(self.size)),
        }
    }
}

/// FNV-1a, 64 bits: a small, fixed, platform-independent hash for "is this the same ship file?"
/// (not a security check: a relay that lied about its ship only fools its own players).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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
    /// Total electrical draw of every switched-ON placed light, in watts
    /// (v0.967, homestead increment 5). `watts_of` maps a light type id to
    /// its draw (the caller passes the light_types.ron lookup; taking a
    /// closure keeps this module renderer-free and unit-testable). Lights
    /// with `on: false` cost nothing - flipping a switch changes the bill.
    pub fn lighting_watts(&self, watts_of: impl Fn(&str) -> f32) -> f32 {
        self.zones
            .iter()
            .flat_map(|z| z.body.lights.iter())
            .filter(|l| l.on)
            .map(|l| watts_of(&l.type_id))
            .sum()
    }

    /// Structural sanity: at least one zone, every id non-empty and unique, every corridor
    /// resolvable (zones exist, from != to, a clear axis gap between the boxes, the door mouth
    /// inside both zones' shared lateral span). Run on every load so a hand-edited file fails
    /// loudly instead of machines clamping into the wrong box or a hallway floating unattached.
    /// (The in-editor SAVE path prunes broken corridor rows first -- `prune_invalid_corridors` --
    /// so a file written by the editor always re-loads.)
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
        self.validate_plots()?;
        self.validate_overlaps()
    }

    /// Plot records: ids non-empty and unique, a size, a parent that exists and contains its
    /// child, a door corridor that resolves from the plot box to a shared zone, a default plot
    /// that exists, and (when assembled) a home that fits inside the plot it was put on.
    fn validate_plots(&self) -> Result<(), String> {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for p in &self.plots {
            if p.id.trim().is_empty() {
                return Err("a plot has an empty id".to_string());
            }
            if !seen.insert(p.id.as_str()) {
                return Err(format!("duplicate plot id '{}'", p.id));
            }
            if !(p.size.0 > 0.0 && p.size.1 > 0.0 && p.size.2 > 0.0) {
                return Err(format!("plot '{}' has no size: {:?}", p.id, p.size));
            }
            if let Some(parent) = &p.parent {
                let outer = self
                    .plots
                    .iter()
                    .find(|q| &q.id == parent && q.id != p.id)
                    .ok_or_else(|| format!("plot '{}' names parent '{parent}', which is not a plot", p.id))?;
                if !box_contains(&outer.aabb(), &p.aabb()) {
                    return Err(format!("plot '{}' is not inside its parent '{parent}'", p.id));
                }
            }
            if p.door.zone == HOME_ZONE_ID {
                return Err(format!("plot '{}' has its door on the home zone; a door goes to a shared zone", p.id));
            }
            self.plot_door_geometry(p).map_err(|e| format!("plot '{}' door: {e}", p.id))?;
        }
        if let Some(d) = &self.default_plot {
            if !self.plots.iter().any(|p| &p.id == d) {
                return Err(format!("default_plot '{d}' is not a plot"));
            }
        }
        // (A home put away stands on no plot: `put_home_away`.)
        if let Some(a) = self.home.as_ref().filter(|a| !a.away) {
            let plot = self
                .plots
                .iter()
                .find(|p| p.id == a.plot)
                .ok_or_else(|| format!("the home was assembled at plot '{}', which is not a plot", a.plot))?;
            let home = self
                .zones
                .iter()
                .find(|z| z.id == HOME_ZONE_ID)
                .ok_or_else(|| "an assembled ship has no home zone".to_string())?;
            if !box_contains(&plot.aabb(), &EndBox::of_zone(home).aabb()) {
                return Err(format!("the home does not fit inside its plot '{}'", plot.id));
            }
        }
        Ok(())
    }

    /// No two of these share volume: plots, zones (the assembled home stands for its plot, so it
    /// is not listed twice) and corridor tubes, including the tube every plot's door WOULD make
    /// when its home is not the one assembled. Touching faces are fine (a tube's mouth sits on
    /// a zone's wall). Exempt: a child plot inside its parent, and the assembled home's own
    /// corridor against the home's own plot. Districts are areas and are not checked.
    fn validate_overlaps(&self) -> Result<(), String> {
        let assembled_plot = self.home.as_ref().map(|a| a.plot.as_str());
        // (label, box, plot id it belongs to, is a tube from that plot's home)
        let mut items: Vec<(String, (Vec3, Vec3), Option<&str>, bool)> = Vec::new();
        for p in &self.plots {
            items.push((format!("plot '{}'", p.id), p.aabb(), Some(p.id.as_str()), false));
        }
        for z in &self.zones {
            if assembled_plot.is_some() && z.id == HOME_ZONE_ID {
                continue;
            }
            items.push((format!("zone '{}'", z.id), EndBox::of_zone(z).aabb(), None, false));
        }
        for c in &self.corridors {
            let Ok(g) = self.corridor_geometry(c) else { continue };
            let home_end = c.from_zone == HOME_ZONE_ID || c.to_zone == HOME_ZONE_ID;
            let owner = if home_end { assembled_plot } else { None };
            items.push((format!("the corridor {} -> {}", c.from_zone, c.to_zone), tube_aabb(&g), owner, owner.is_some()));
        }
        for p in &self.plots {
            if Some(p.id.as_str()) == assembled_plot {
                continue; // its tube is the real home corridor above
            }
            if let Ok(g) = self.plot_door_geometry(p) {
                items.push((format!("the corridor from plot '{}' to {}", p.id, p.door.zone), tube_aabb(&g), Some(p.id.as_str()), true));
            }
        }
        for (i, a) in items.iter().enumerate() {
            for b in items.iter().skip(i + 1) {
                if !boxes_overlap(&a.1, &b.1) {
                    continue;
                }
                // A plot's own door tube against that plot (the home's face is inside a larger
                // plot), or against a block plot it sits in (a cabin's corridor runs through its
                // block), is not an overlap.
                let tube_in_own = |tube: &(String, (Vec3, Vec3), Option<&str>, bool), other: &(String, (Vec3, Vec3), Option<&str>, bool)| {
                    tube.3 && !other.3 && matches!((tube.2, other.2), (Some(t), Some(o)) if self.is_self_or_ancestor(o, t))
                };
                let own_tube = tube_in_own(a, b) || tube_in_own(b, a);
                // A child plot inside its parent.
                let nested = match (a.2, b.2, a.3 || b.3) {
                    (Some(x), Some(y), false) => self.plots.iter().any(|p| {
                        (p.id == x && p.parent.as_deref() == Some(y)) || (p.id == y && p.parent.as_deref() == Some(x))
                    }),
                    _ => false,
                };
                if !own_tube && !nested {
                    return Err(format!("{} overlaps {}", a.0, b.0));
                }
            }
        }
        Ok(())
    }

    /// True when plot `outer` is plot `inner` or one of its parents (a bounded walk, so a
    /// parent loop in hand-edited data cannot hang it).
    fn is_self_or_ancestor(&self, outer: &str, inner: &str) -> bool {
        let mut cur = Some(inner);
        for _ in 0..=self.plots.len() {
            match cur {
                Some(id) if id == outer => return true,
                Some(id) => cur = self.plots.iter().find(|p| p.id == id).and_then(|p| p.parent.as_deref()),
                None => return false,
            }
        }
        false
    }

    /// The tube a plot's door corridor makes from the plot box to its door zone.
    fn plot_door_geometry(&self, p: &Plot) -> Result<CorridorGeom, String> {
        let to_idx = self
            .zone_index(&p.door.zone)
            .ok_or_else(|| format!("unknown zone '{}'", p.door.zone))?;
        let row = p.door.corridor_from(&p.id);
        tube_between(p.end_box(), EndBox::of_zone(&self.zones[to_idx]), &row, usize::MAX, to_idx)
    }

    /// Resolve a corridor row to world geometry, or the honest reason it cannot exist. This IS the
    /// corridor validator: `validate` (load), the editor's Create button, mesh generation, and
    /// collision all go through it, so they can never disagree about what a corridor is.
    ///
    /// The resolve is PURELY box-vs-box: run axis = the axis with the larger clear gap between the
    /// two zone footprints, start/end = the facing perimeter planes, centreline = the row's own
    /// `lat`. Authored doors never enter the computation -- that independence is the whole point
    /// of the rework (the operator's desync bug: door-list indices shifted under every door edit).
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
        tube_between(
            EndBox::of_zone(&self.zones[from_idx]),
            EndBox::of_zone(&self.zones[to_idx]),
            c,
            from_idx,
            to_idx,
        )
    }
}

/// Whether a home `design` fits `plot`'s box (its width, height and depth, each within the
/// overlap tolerance), with the reason when not. One rule for the player's own home (`assemble`
/// refuses such a pairing) and a neighbour's (src/ship/neighbours.rs leaves such a plot undrawn,
/// increment 2 review, finding 7: it was drawn anyway, spilling into the next plot or zone).
pub(crate) fn design_fits_plot(design: &HomeDesign, plot: &Plot) -> Result<(), String> {
    let b = &design.body;
    if b.width > plot.size.0 + OVERLAP_EPS || b.height > plot.size.1 + OVERLAP_EPS || b.depth > plot.size.2 + OVERLAP_EPS {
        return Err(format!(
            "the {} design ({} x {} x {} m) does not fit plot '{}' ({:?})",
            design.kind, b.width, b.depth, b.height, plot.id, plot.size
        ));
    }
    Ok(())
}

/// The door-sized APERTURE a corridor tube makes where one of its ends meets a box (min corner
/// `o`, `w` x `d` footprint, `h` tall): `end` is the tube's mouth on that box, `other` the mouth
/// at its far end. The tube leaves the box through the perimeter face in the run direction toward
/// the other end. `at` converts the world `lat` centreline to edge-local metres, honouring each
/// edge's WINDING (verified against `generate_meshes_with_shell_cuts`'s perimeter build order,
/// documented on `ShellCut`): 0 runs +x along z=0, 1 runs +z along x=w, 2 runs -x along z=d, 3
/// runs -z along x=0. Shared by a zone's own cuts (`shell_cuts_for_zone`) and the render-only
/// cuts a neighbour's corridor makes (src/ship/neighbours.rs), so the two open the same hole.
pub(crate) fn end_mouth_cut(o: Vec3, w: f32, d: f32, h: f32, g: &CorridorGeom, end: Vec3, other: Vec3) -> ShellCut {
    let (dw, dh) = g.door_from;
    let (edge, at) = match g.axis {
        CorridorAxis::X if other.x > end.x => (1usize, (g.lat - o.z) - dw * 0.5),
        CorridorAxis::X => (3, d - ((g.lat - o.z) + dw * 0.5)),
        CorridorAxis::Z if other.z > end.z => (2, w - ((g.lat - o.x) + dw * 0.5)),
        CorridorAxis::Z => (0, (g.lat - o.x) - dw * 0.5),
    };
    // The cut's foot is the corridor's deck, body-local (0 while corridors run level with the
    // boxes they join, which every shipped one does).
    ShellCut { edge, at, width: dw, base: g.floor_y - o.y, height: dh.min(h) }
}

/// The corridor resolve on two BOXES (zones, or a plot standing in for its home): the
/// validator body of `ShipStructure::corridor_geometry`, shared with the plot-door check so a
/// plot's would-be corridor obeys exactly the rules a real one does.
fn tube_between(
    from: EndBox,
    to: EndBox,
    c: &ShipCorridor,
    from_idx: usize,
    to_idx: usize,
) -> Result<CorridorGeom, String> {
    {
        if c.width < CORRIDOR_MIN_WIDTH {
            return Err(format!(
                "width {:.2} m is below the {CORRIDOR_MIN_WIDTH} m minimum",
                c.width
            ));
        }
        if c.door_width < CORRIDOR_MIN_WIDTH {
            return Err(format!(
                "door width {:.2} m is below the {CORRIDOR_MIN_WIDTH} m minimum",
                c.door_width
            ));
        }
        if c.door_width > c.width + 1e-4 {
            return Err(format!(
                "door width {:.2} m exceeds the tube width {:.2} m; the tube must enclose its own mouth",
                c.door_width, c.width
            ));
        }
        if c.door_height < CORRIDOR_MIN_DOOR_HEIGHT {
            return Err(format!(
                "door height {:.2} m is below the {CORRIDOR_MIN_DOOR_HEIGHT} m minimum",
                c.door_height
            ));
        }
        let of = from.origin;
        let ot = to.origin;
        if (of.y - ot.y).abs() > 0.01 {
            return Err(format!(
                "zones are at different deck heights ({:.2} vs {:.2} m); v1 corridors are level",
                of.y, ot.y
            ));
        }
        // World-XZ footprints (min, max) of both zone boxes -- the ONLY inputs to the run.
        let (f_min, f_max) = ((of.x, of.z), (of.x + from.w, of.z + from.d));
        let (t_min, t_max) = ((ot.x, ot.z), (ot.x + to.w, ot.z + to.d));
        // Clear gap per axis: positive when the boxes have open air between them on that axis
        // (whichever side the other zone is on), negative when their spans overlap.
        let gap_x = (t_min.0 - f_max.0).max(f_min.0 - t_max.0);
        let gap_z = (t_min.1 - f_max.1).max(f_min.1 - t_max.1);
        let axis = if gap_x >= gap_z { CorridorAxis::X } else { CorridorAxis::Z };
        if gap_x.max(gap_z) < CORRIDOR_MIN_RUN {
            return Err(format!(
                "zones '{}' and '{}' overlap or touch -- no corridor run (a straight tube needs a \
                 clear gap of at least {CORRIDOR_MIN_RUN} m on one world axis)",
                c.from_zone, c.to_zone
            ));
        }
        // start/end = the two FACING perimeter planes on the run axis (start < end always);
        // remember which plane belongs to the from zone so end_from lands on ITS shell.
        let (start, end, from_at_start) = match axis {
            CorridorAxis::X if t_min.0 - f_max.0 >= f_min.0 - t_max.0 => (f_max.0, t_min.0, true),
            CorridorAxis::X => (t_max.0, f_min.0, false),
            CorridorAxis::Z if t_min.1 - f_max.1 >= f_min.1 - t_max.1 => (f_max.1, t_min.1, true),
            CorridorAxis::Z => (t_max.1, f_min.1, false),
        };
        // The centreline must land the WHOLE door mouth inside both zones' spans on the axis
        // across the run, or a cut would run off the end of a perimeter wall.
        let (f_lo, f_hi, t_lo, t_hi, perp) = match axis {
            CorridorAxis::X => (f_min.1, f_max.1, t_min.1, t_max.1, "z"),
            CorridorAxis::Z => (f_min.0, f_max.0, t_min.0, t_max.0, "x"),
        };
        let (lo, hi) = (f_lo.max(t_lo), f_hi.min(t_hi));
        if hi <= lo {
            return Err(format!(
                "the zones do not overlap on the {perp} axis; a straight {} corridor cannot \
                 connect them (L-bends are a follow-up)",
                axis.name()
            ));
        }
        let half_door = c.door_width * 0.5;
        let (lat_lo, lat_hi) = (lo + half_door, hi - half_door);
        if lat_hi < lat_lo {
            return Err(format!(
                "the zones share only {:.2} m of {perp} span; too narrow for a {:.2} m door",
                hi - lo,
                c.door_width
            ));
        }
        if c.lat < lat_lo - 1e-4 || c.lat > lat_hi + 1e-4 {
            return Err(format!(
                "lat {:.2} m puts the door mouth outside the zones' shared {perp} span; valid: \
                 {:.2} to {:.2} m",
                c.lat, lat_lo, lat_hi
            ));
        }
        let floor_y = of.y;
        let height = from.deck_h.min(to.deck_h).max(1.0);
        // Both mouths are the corridor's OWN door, clamped so the header never pokes above the lid.
        let door = (c.door_width, c.door_height.min(height));
        let (fa, ta) = if from_at_start { (start, end) } else { (end, start) };
        let (end_from, end_to) = match axis {
            CorridorAxis::X => (
                Vec3::new(fa, floor_y, c.lat),
                Vec3::new(ta, floor_y, c.lat),
            ),
            CorridorAxis::Z => (
                Vec3::new(c.lat, floor_y, fa),
                Vec3::new(c.lat, floor_y, ta),
            ),
        };
        Ok(CorridorGeom {
            axis,
            start,
            end,
            lat: c.lat,
            floor_y,
            height,
            width: c.width,
            glass_top: c.glass_top,
            end_from,
            end_to,
            from_zone_idx: from_idx,
            to_zone_idx: to_idx,
            door_from: door,
            door_to: door,
        })
    }
}

impl ShipStructure {
    /// The valid `lat` range for a corridor row -- the centreline positions that keep the whole
    /// door mouth inside both zones' shared span across the run -- or None when the pair cannot
    /// host a corridor at all (unknown zone, no clear axis gap, shared span too narrow for the
    /// door). Mirrors `corridor_geometry`'s box math WITHOUT the error strings so the viewport
    /// mouth-drag (v0.790) can CLAMP a drag to legal positions instead of writing a lat the
    /// resolver would reject. `corridor_geometry` stays the single validator for everything
    /// else: a dragged row still re-resolves through it on every rebuild, so even a disagreement
    /// here would only skip that corridor's mesh (the Corridors panel shows why), never crash.
    pub fn corridor_lat_limits(&self, c: &ShipCorridor) -> Option<(f32, f32)> {
        let zf = &self.zones[self.zone_index(&c.from_zone)?];
        let zt = &self.zones[self.zone_index(&c.to_zone)?];
        let of = zf.origin_vec();
        let ot = zt.origin_vec();
        // World-XZ footprints (min, max) of both zone boxes -- the same inputs the resolver uses.
        let (f_min, f_max) = ((of.x, of.z), (of.x + zf.body.width, of.z + zf.body.depth));
        let (t_min, t_max) = ((ot.x, ot.z), (ot.x + zt.body.width, ot.z + zt.body.depth));
        let gap_x = (t_min.0 - f_max.0).max(f_min.0 - t_max.0);
        let gap_z = (t_min.1 - f_max.1).max(f_min.1 - t_max.1);
        if gap_x.max(gap_z) < CORRIDOR_MIN_RUN {
            return None; // boxes overlap/touch (also covers from == to) -- no run, no lat range
        }
        let axis = if gap_x >= gap_z { CorridorAxis::X } else { CorridorAxis::Z };
        // The zones' spans on the axis ACROSS the run; the mouth must fit inside the overlap.
        let (f_lo, f_hi, t_lo, t_hi) = match axis {
            CorridorAxis::X => (f_min.1, f_max.1, t_min.1, t_max.1),
            CorridorAxis::Z => (f_min.0, f_max.0, t_min.0, t_max.0),
        };
        let (lo, hi) = (f_lo.max(t_lo), f_hi.min(t_hi));
        let half_door = c.door_width * 0.5;
        let (lat_lo, lat_hi) = (lo + half_door, hi - half_door);
        if lat_hi < lat_lo {
            return None; // shared span narrower than the door
        }
        Some((lat_lo, lat_hi))
    }

    /// The corridor APERTURES through zone `zi`'s perimeter shell: for each valid corridor ending
    /// in this zone, one door-sized cut through the perimeter face its tube leaves by (the face in
    /// the run direction toward the other zone). The cut is the corridor's OWN door mouth
    /// (`door_width` centred on `lat`) -- since the rework, the generated shell aperture IS the
    /// only wall at the mouth (the coincident authored door walls were deleted with it, killing
    /// the operator's z-fighting + walk-through-wall bug). The tube (which may be wider) encloses
    /// the cut from outside. Fed to `generate_meshes_with_shell_cuts` (mesh) and
    /// `wall_segments_with_shell_cuts` (collision).
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
            // Which end of this corridor (if either) lands in zone `zi`. Both mouths share the
            // corridor-owned door size (door_from == door_to since the rework).
            let (end, other) = if g.from_zone_idx == zi {
                (g.end_from, g.end_to)
            } else if g.to_zone_idx == zi {
                (g.end_to, g.end_from)
            } else {
                // NOT an end zone -- but the tube may still CROSS this zone's
                // perimeter (v0.789, operator: "there's still a wall in the
                // corridor"). Zones legitimately overlap in this ship design
                // (his 120x200 m Residential region contains both corridor
                // ends), so any perimeter plane an intervening zone puts across
                // the tube's path gets the same door-sized cut the end mouths
                // get. Collision uses these same cuts, so the passage is
                // walkable too.
                let (dw, dh) = g.door_from;
                // `start`/`end` are the run-axis coordinates of the two mouths.
                let (lo, hi) = (g.start.min(g.end), g.start.max(g.end));
                match g.axis {
                    CorridorAxis::X => {
                        // The door must fit inside this zone's z-span at lat.
                        if g.lat - dw * 0.5 >= o.z && g.lat + dw * 0.5 <= o.z + d {
                            // West face (x = o.x) is edge 3; east face (x = o.x + w) is edge 1.
                            for (plane, edge, at) in [
                                (o.x, 3usize, d - ((g.lat - o.z) + dw * 0.5)),
                                (o.x + w, 1, (g.lat - o.z) - dw * 0.5),
                            ] {
                                if plane > lo + 0.01 && plane < hi - 0.01 {
                                    cuts.push(ShellCut {
                                        edge,
                                        at,
                                        width: dw,
                                        base: g.floor_y - o.y,
                                        height: dh.min(zone.body.height),
                                    });
                                }
                            }
                        }
                    }
                    CorridorAxis::Z => {
                        if g.lat - dw * 0.5 >= o.x && g.lat + dw * 0.5 <= o.x + w {
                            // North face (z = o.z) is edge 0; south face (z = o.z + d) is edge 2.
                            for (plane, edge, at) in [
                                (o.z, 0usize, (g.lat - o.x) - dw * 0.5),
                                (o.z + d, 2, w - ((g.lat - o.x) + dw * 0.5)),
                            ] {
                                if plane > lo + 0.01 && plane < hi - 0.01 {
                                    cuts.push(ShellCut {
                                        edge,
                                        at,
                                        width: dw,
                                        base: g.floor_y - o.y,
                                        height: dh.min(zone.body.height),
                                    });
                                }
                            }
                        }
                    }
                }
                continue;
            };
            cuts.push(end_mouth_cut(o, w, d, zone.body.height, &g, end, other));
        }
        cuts
    }

    /// Every corridor door-mouth plane in world space (corridor door panels, v0.795): both tube
    /// ends + any intervening-zone perimeter crossings -- exactly the planes `shell_cuts_for_zone`
    /// opens apertures through, with the same predicates and clamps (keep the two in lockstep; see
    /// `CorridorMouth`). `ship::door_panels::corridor_panel_placements` builds the sliding door
    /// pair from each mouth; mesh + collision keep consuming the per-zone cuts unchanged.
    pub fn corridor_mouths(&self) -> Vec<CorridorMouth> {
        let mut out = Vec::new();
        for c in &self.corridors {
            let Ok(g) = self.corridor_geometry(c) else {
                continue; // a broken row gets no doors (mesh + collision skip it too)
            };
            out.extend(self.tube_mouths(&g));
        }
        out
    }

    /// One resolved tube's door mouths (`corridor_mouths`, for each of the ship's corridors; and
    /// a neighbour's corridor, src/ship/neighbours.rs `neighbour_mouths`, whose doors open only
    /// for the other players): its two end mouths and any intervening-zone crossings.
    pub fn tube_mouths(&self, g: &CorridorGeom) -> Vec<CorridorMouth> {
        let mut out = Vec::new();
        {
            // Where THIS corridor's mouths start, for the coincident-plane dedup below (two
            // separate corridors may legitimately share a plane at different lats).
            let first = out.len();
            let (dw, dh) = g.door_from;
            // The two end mouths, on the zones' facing perimeter planes.
            for plane in [g.start, g.end] {
                out.push(CorridorMouth {
                    axis: g.axis,
                    plane,
                    lat: g.lat,
                    floor_y: g.floor_y,
                    door: (dw, dh),
                });
            }
            // Intervening-zone crossings (the v0.789 shell-cut case: a big region zone overlapping
            // the run puts its own perimeter across the tube): a perimeter plane STRICTLY between
            // the two end mouths, with the whole door inside that zone's span across the run.
            for (zi, zone) in self.zones.iter().enumerate() {
                if zi == g.from_zone_idx || zi == g.to_zone_idx {
                    continue;
                }
                let o = zone.origin_vec();
                let (w, d) = (zone.body.width, zone.body.depth);
                let (lo, hi) = (g.start.min(g.end), g.start.max(g.end));
                // The zone's two candidate perimeter planes on the run axis + whether the door
                // mouth fits inside its span on the axis ACROSS the run (same test the cut does).
                let (planes, fits) = match g.axis {
                    CorridorAxis::X => (
                        [o.x, o.x + w],
                        g.lat - dw * 0.5 >= o.z && g.lat + dw * 0.5 <= o.z + d,
                    ),
                    CorridorAxis::Z => (
                        [o.z, o.z + d],
                        g.lat - dw * 0.5 >= o.x && g.lat + dw * 0.5 <= o.x + w,
                    ),
                };
                if !fits {
                    continue;
                }
                for plane in planes {
                    // Strictly inside the run (an end-coincident plane already has its end mouth),
                    // and deduped against this corridor's other mouths: two intervening zones with
                    // COPLANAR faces across the tube must not stack two door pairs in one aperture
                    // (z-fighting panels). The shell cuts don't dedup -- each zone cuts its OWN
                    // shell -- but the doors live in the shared world aperture, so one pair serves.
                    if plane > lo + 0.01
                        && plane < hi - 0.01
                        && !out[first..].iter().any(|m| (m.plane - plane).abs() < 0.01)
                    {
                        out.push(CorridorMouth {
                            axis: g.axis,
                            plane,
                            lat: g.lat,
                            floor_y: g.floor_y,
                            door: (dw, dh.min(zone.body.height)),
                        });
                    }
                }
            }
        }
        out
    }

    /// Drop corridor rows that no longer resolve (a referenced zone was deleted/renamed, or an
    /// edit dragged the boxes to overlap / the mouth out of the shared span). Returns how many
    /// were dropped. Called by the engine's
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

    /// Where the player stands on world entry: the home zone's authored spawn point at eye
    /// height (1.7 m), in ship metres. The authored point is home-local, so it rides the home's
    /// origin, which is its plot's (before increment 1a it was used as a ship position as it
    /// stood, which only worked because the one home sat at the origin). None when the home
    /// declares no spawn.
    pub fn home_spawn_world(&self) -> Option<Vec3> {
        let home = self.zones.get(self.home_zone_index())?;
        Some(spawn_at(home.origin, home.body.spawn?))
    }

    /// The plot the home was assembled at (None for a ship file on its own).
    pub fn home_plot(&self) -> Option<&Plot> {
        // A home put away stands on no plot (`put_home_away`).
        let a = self.home.as_ref().filter(|a| !a.away)?;
        self.plots.iter().find(|p| p.id == a.plot)
    }

    /// Where the holder of `plot` arrives with `design` on it (increment 1b): the design's spawn
    /// at the plot's origin, exactly what `home_spawn_world` gives once that design is assembled
    /// there; the middle of the plot at eye height when the design declares no spawn. The relay
    /// spawns the player here and the game puts its camera here, from this one function.
    pub fn plot_spawn(plot: &Plot, design: &HomeDesign) -> Vec3 {
        let local = design.body.spawn.unwrap_or_else(|| plot_middle(plot.size));
        spawn_at(plot.origin, local)
    }

    /// Where this player's own home is entered, as plot-local x and z metres: what the game
    /// sends in `game_join` (`home_spawn`) so the relay spawns them at their OWN door
    /// (`PlotArrival::arrival`). Plot-local because the game does not know its plot until the
    /// welcome, and the home design is the same on any plot. None for a ship that was not
    /// assembled with a home, AND for a home with no authored door: then no door is named and
    /// each side takes the middle of the plot actually handed out (the second review of 1b:
    /// this used to send the middle of the plot the home was built on, which agreed with the
    /// relay only while every plot was the same size).
    pub fn home_arrival_local(&self) -> Option<(f32, f32)> {
        // The home zone stands at its plot's origin (`assemble`), so its body's spawn, which is
        // home-local, is plot-local too. A home put away names its door as well (increment 2):
        // the door is the design's, and a relay may hand a returning guest a plot.
        self.home.as_ref()?;
        self.zones.get(self.home_zone_index())?.body.spawn
    }

    /// Where a guest arrives when every plot is held (increment 1b): the Commons (the first
    /// zone whose purpose is "commons"), at its spawn or its middle, at eye height. None for a
    /// ship with no Commons.
    pub fn guest_spawn(&self) -> Option<Vec3> {
        let c = self.zones.iter().find(|z| z.purpose == "commons")?;
        Some(spawn_at(c.origin, c.body.spawn.unwrap_or_else(|| plot_middle((c.body.width, 0.0, c.body.depth)))))
    }

    /// A short fingerprint of the ship FILE (the shared ship, never anyone's home): 16 hex digits
    /// of FNV-1a over its compact RON. The relay sends it in `game_welcome` and the game compares
    /// its own, because positions only agree when everyone has the same ship (increment 1b). It
    /// is taken from the parsed file, so comments and number formatting do not change it, and
    /// from `ship_file()`, so an assembled ship and the file it came from give the same value.
    pub fn ship_hash(&self) -> String {
        let text = ron::ser::to_string(&self.ship_file()).unwrap_or_default();
        format!("{:016x}", fnv1a64(text.as_bytes()))
    }

    /// The origin of the zone a machine row names, with the same fallback the placer uses
    /// (`machines::resolve_zone_rect`: the named zone, else "home", else the first zone).
    /// Machine offsets are zone-local since increment 1a, so the editor adds this to show a
    /// machine and subtracts it when a drag writes one back.
    pub fn machine_zone_origin(&self, zone_id: &str) -> Vec3 {
        self.zones
            .iter()
            .find(|z| z.id == zone_id)
            .or_else(|| self.zones.iter().find(|z| z.id == HOME_ZONE_ID))
            .or_else(|| self.zones.first())
            .map(|z| z.origin_vec())
            .unwrap_or(Vec3::ZERO)
    }

    /// The plot offline play uses: `default_plot`, else the first plot.
    pub fn default_plot_id(&self) -> Option<String> {
        self.default_plot.clone().or_else(|| self.plots.first().map(|p| p.id.clone()))
    }

    /// The plot a home of `kind` goes on when nothing names one (ship homes increment 6: a cabin
    /// save with no place yet): `default_plot` when it is of that kind, else the first plot of that
    /// kind in the file. None when the ship has no plot of that kind.
    pub fn default_plot_of_kind(&self, kind: &str) -> Option<String> {
        let of_kind = |id: &str| self.plots.iter().any(|p| p.id == id && p.kind == kind);
        self.default_plot
            .clone()
            .filter(|d| of_kind(d))
            .or_else(|| self.plots.iter().find(|p| p.kind == kind).map(|p| p.id.clone()))
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
        // Plots count too: a new zone must not land on someone's home.
        let max_x = self
            .zones
            .iter()
            .map(|z| z.origin.0 + z.body.width)
            .chain(self.plots.iter().map(|p| p.origin.0 + p.size.0))
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

    /// World AABB of all zone boxes (min, max) -- the conduit-node clamp bounds. A home put away
    /// is no part of the ship (`put_home_away`).
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::INFINITY);
        let mut mx = Vec3::splat(f32::NEG_INFINITY);
        for (_, z) in self.zones.iter().enumerate().filter(|(zi, _)| !self.is_away_home(*zi)) {
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

    /// Load from RON. None (with a loud warning + quarantine) only when the file is truly
    /// unusable -- the caller falls back exactly as it did for a broken home_structure.ron.
    ///
    /// RESILIENCE (v0.791, the v0.788-migration lesson): a rejected load silently reverts the
    /// player's ENTIRE ship to the shipped default, which reads as "all my saves are gone".
    /// So a validate failure no longer costs the whole file: unresolvable corridors are
    /// pruned (same rule as save) and every zone is kept. Only a parse error or zone-level
    /// invalidity (duplicate/empty ids -- unreachable through the editor) still falls back,
    /// and then the bad file is QUARANTINED (renamed, never overwritten) so the player's
    /// data stays recoverable instead of being clobbered by the next Save.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        match ron::from_str::<ShipStructure>(&text) {
            Ok(mut s) => match s.validate() {
                Ok(()) => Some(s),
                Err(e) => {
                    let pruned = s.prune_invalid_corridors();
                    match s.validate() {
                        Ok(()) => {
                            log::warn!(
                                "ship_structure: {} had {pruned} unresolvable corridor(s) ({e}); dropped them and kept every zone",
                                path.display()
                            );
                            Some(s)
                        }
                        Err(e2) => {
                            quarantine(path, &format!("invalid beyond corridor pruning: {e2}"));
                            None
                        }
                    }
                }
            },
            Err(e) => {
                quarantine(path, &format!("failed to parse: {e}"));
                None
            }
        }
    }

    /// The ship file from a data dir: disk first (`load`, which quarantines a file it cannot
    /// use), else the copy built into the exe, so a fresh install or a throwaway relay has the
    /// same ship. Err says why there is no ship.
    pub fn load_ship_file(data_dir: &Path) -> Result<ShipStructure, String> {
        let path = data_dir.join(SHIP_FILE);
        if path.exists() {
            return Self::load(&path).ok_or_else(|| {
                format!(
                    "{} did not load; it was moved aside as ship_structure.invalid-<time>.ron (see logs/run.log)",
                    path.display()
                )
            });
        }
        Self::built_in_ship_file(&format!("{} is absent", path.display()))
            .map_err(|e| format!("no {} on disk, and {e}", path.display()))
    }

    /// The ship file a RELAY builds its world from (increment 3 of
    /// docs/design/ship-homes-and-logistics.md: its plots, and since then its rooms, crew and
    /// stores too): the file on disk, else the copy built into the exe. A file on disk that does
    /// not load gives way to the built-in copy here, so a relay keeps the ship the same version
    /// of the game draws (the third review of 1b: it used to keep NO ship, and every game's join
    /// was then refused as "a different ship"). Err only when neither loads.
    pub fn ship_for_relay(data_dir: &Path) -> Result<ShipStructure, String> {
        match Self::load_ship_file(data_dir) {
            Ok(ship) => Ok(ship),
            Err(e) => {
                let ship = Self::built_in_ship_file(&format!("ship plots: {e}")).map_err(|b| format!("{e}; {b}"))?;
                log::error!("ship plots: {e}; using the ship built into the exe ({})", ship.id);
                Ok(ship)
            }
        }
    }

    /// The ship file built into the exe (data/blueprints/ship_structure.ron as shipped).
    /// Both callers use it as a FALLBACK (the disk file is absent, or it did not load), so it
    /// says so itself with `embedded_data::note_builtin_copy` and `why`: a rig run on the
    /// built-in copy then fails instead of passing on data it was not given (BUG-133
    /// follow-up; it is on FALLBACK_SITES in scripts/lib/compiled-in.js).
    pub fn built_in_ship_file(why: &str) -> Result<ShipStructure, String> {
        let text = crate::embedded_data::get_embedded(SHIP_FILE).ok_or("no ship file is built in")?;
        crate::embedded_data::note_builtin_copy(SHIP_FILE, why);
        let ship: ShipStructure = ron::from_str(text).map_err(|e| format!("the built-in ship file does not parse: {e}"))?;
        ship.validate().map_err(|e| format!("the built-in ship file is invalid: {e}"))?;
        Ok(ship)
    }

    /// Put a home design on a plot of this ship file: the design's body becomes zone `home` at
    /// the plot's origin (first in the zone list, as the home always was), the plot's door
    /// becomes the home's corridor (first in the corridor list, so it keeps the id
    /// `corridor_0`), and the result is validated. Refused, with the reason: a ship file that
    /// already holds a home, an unknown plot, a design of another kind, a body bigger than the
    /// plot, or a plot door that does not meet the design's door point.
    pub fn assemble(mut self, design: HomeDesign, plot_id: &str) -> Result<ShipStructure, String> {
        if self.zone_index(HOME_ZONE_ID).is_some() {
            return Err(format!(
                "the ship file has a zone named '{HOME_ZONE_ID}'; homes live in {HOME_DESIGNS_DIR}/ and go on plots (increment 1a)"
            ));
        }
        if self.corridors.iter().any(|c| c.from_zone == HOME_ZONE_ID || c.to_zone == HOME_ZONE_ID) {
            return Err(format!("the ship file has a corridor to '{HOME_ZONE_ID}'; a plot's door makes that corridor"));
        }
        let plot = self
            .plots
            .iter()
            .find(|p| p.id == plot_id)
            .cloned()
            .ok_or_else(|| format!("the ship has no plot '{plot_id}'"))?;
        if plot.kind != design.kind {
            return Err(format!("plot '{}' takes a {} home, not a {}", plot.id, plot.kind, design.kind));
        }
        design_fits_plot(&design, &plot)?;
        let b = &design.body;
        let lat = design.door_lat(plot.origin)?;
        if (lat - plot.door.lat).abs() > OVERLAP_EPS {
            return Err(format!(
                "plot '{}' door lat {} does not meet the {} design's door at local {:?}, which needs lat {lat}",
                plot.id, plot.door.lat, design.kind, design.door
            ));
        }
        // The corridor must leave the home THROUGH the design's door, not through another wall
        // that happens to share its lat: a plot east of its street, with the door in the home's
        // east wall, gives the right lat but a corridor that cuts the west wall, into whatever
        // room is there, while the real door opens onto nothing (the critic's review of 1a).
        // So the tube's home-end mouth must be the door point itself, in ship metres.
        if let Some(to_idx) = self.zone_index(&plot.door.zone) {
            let home_box = EndBox::of_body(Vec3::new(plot.origin.0, plot.origin.1, plot.origin.2), b);
            let row = plot.door.corridor_from(HOME_ZONE_ID);
            if let Ok(g) = tube_between(home_box, EndBox::of_zone(&self.zones[to_idx]), &row, 0, to_idx) {
                let door = (plot.origin.0 + design.door.0, plot.origin.2 + design.door.1);
                if (g.end_from.x - door.0).abs() > OVERLAP_EPS || (g.end_from.z - door.1).abs() > OVERLAP_EPS {
                    return Err(format!(
                        "plot '{}' door corridor to '{}' leaves the home at ({:.1}, {:.1}), not through the {} design's door at ({:.1}, {:.1})",
                        plot.id, plot.door.zone, g.end_from.x, g.end_from.z, design.kind, door.0, door.1
                    ));
                }
            }
        }
        self.zones.insert(
            0,
            ShipZone {
                id: HOME_ZONE_ID.to_string(),
                label: "Player Home".to_string(),
                purpose: "residence".to_string(),
                origin: plot.origin,
                body: design.body,
            },
        );
        self.corridors.insert(0, plot.door.corridor_from(HOME_ZONE_ID));
        self.home = Some(HomeAssembly { plot: plot.id.clone(), kind: design.kind, door: design.door, away: false });
        self.validate()?;
        Ok(self)
    }

    /// This assembled ship with its home PUT AWAY (increment 2): the player is a guest here, with
    /// no plot of this ship, so their home stands on none. The ship file, plus the home design as
    /// zone `home` at `HOME_AWAY_ORIGIN`, with no door corridor. Every other system keeps the home
    /// it knows (the machines' zone, the save's frame, the editor's zone index): it is moved, not
    /// taken away, and comes back onto a plot through `assemble`. None for a ship that was not
    /// assembled with a home, or whose home is already away.
    pub fn put_home_away(&self) -> Option<ShipStructure> {
        let a = self.home.as_ref().filter(|a| !a.away)?;
        let body = self.zones.iter().find(|z| z.id == HOME_ZONE_ID)?.body.clone();
        let mut ship = self.ship_file();
        ship.zones.insert(
            0,
            ShipZone {
                id: HOME_ZONE_ID.to_string(),
                label: "Player Home".to_string(),
                purpose: "residence".to_string(),
                origin: HOME_AWAY_ORIGIN,
                body,
            },
        );
        ship.home = Some(HomeAssembly { plot: String::new(), kind: a.kind.clone(), door: a.door, away: true });
        ship.validate().ok()?;
        Some(ship)
    }

    /// True while the home is put away (`put_home_away`).
    pub fn home_is_away(&self) -> bool {
        self.home.as_ref().is_some_and(|a| a.away)
    }

    /// True for the zone at `zi` when it is a home put away: not drawn, walked into, lit or
    /// wrapped by the hull (`put_home_away`).
    pub fn is_away_home(&self, zi: usize) -> bool {
        self.home_is_away() && self.zones.get(zi).is_some_and(|z| z.id == HOME_ZONE_ID)
    }

    /// The plots this game draws as its NEIGHBOURS' (increment 2): every plot but the one its home
    /// stands on (all of them while the home is away, or for a ship file on its own), and never a
    /// plot that one sits inside (its block) or that sits inside it: a neighbour drawn there would
    /// put a default shell over the home, or one inside it (increment 2 review, finding 7; plots
    /// may nest, `Plot::parent`, which `validate` allows).
    pub fn neighbour_plots(&self) -> impl Iterator<Item = &Plot> {
        let own = self.home.as_ref().filter(|a| !a.away).map(|a| a.plot.clone());
        self.plots.iter().filter(move |p| match own.as_deref() {
            None => true,
            Some(o) => !self.is_self_or_ancestor(&p.id, o) && !self.is_self_or_ancestor(o, &p.id),
        })
    }

    /// The tube a plot's door corridor makes from a home of `design` standing on it (the home's
    /// box at the plot's origin, as `assemble` puts it), or from the plot's own box when no design
    /// is given. A neighbour's corridor is drawn from this (src/ship/neighbours.rs), and the rig's
    /// door points are read off it (src/ship/door_points.rs), so neither re-does the corridor
    /// rules.
    pub fn plot_door_tube(&self, plot: &Plot, design: Option<&HomeDesign>) -> Result<CorridorGeom, String> {
        let to_idx = self.zone_index(&plot.door.zone).ok_or_else(|| format!("unknown zone '{}'", plot.door.zone))?;
        let from = match design {
            Some(d) => EndBox::of_body(Vec3::new(plot.origin.0, plot.origin.1, plot.origin.2), &d.body),
            None => plot.end_box(),
        };
        tube_between(from, EndBox::of_zone(&self.zones[to_idx]), &plot.door.corridor_from(&plot.id), usize::MAX, to_idx)
    }

    /// The ship file plus the home design of `plot`'s kind on `plot` (the ship's default plot
    /// when None), the home read from `data_dir` disk first: this player's OWN home. The world
    /// load does not call this: it builds on the plot remembered for the server, offline play
    /// included (engine/home_plot.rs `assemble_for_boot`, which calls `assemble_from`). A test
    /// that means the SHIPPED ship calls `load_and_assemble_shipped`: in a repo checkout
    /// data/homes/<kind>.ron is the developer's own home, which an editor Save rewrites.
    pub fn load_and_assemble(data_dir: &Path, plot: Option<&str>) -> Result<ShipStructure, String> {
        Self::assemble_from(Self::load_ship_file(data_dir)?, data_dir, plot)
    }

    /// TESTS ONLY: the ship file from `data_dir` with the SHIPPED design of `plot`'s kind on
    /// `plot` (`HomeDesign::built_in`, data/homes/shipped/), the ship as a newcomer boots it.
    /// Never data/homes/<kind>.ron, which in a repo checkout is the developer's own home: tests
    /// that read it as the default turned red after an editor Save there (the final review of
    /// ship homes increment 2).
    #[cfg(test)]
    pub fn load_and_assemble_shipped(data_dir: &Path, plot: Option<&str>) -> Result<ShipStructure, String> {
        let ship = Self::load_ship_file(data_dir)?;
        let (plot_id, kind) = ship.plot_and_kind(plot)?;
        let design = HomeDesign::built_in(&kind).ok_or_else(|| format!("no {kind} home design is built in"))?;
        ship.assemble(design, &plot_id)
    }

    /// `load_and_assemble` on a ship file already loaded: the home design of `plot`'s kind (from
    /// `data_dir`, disk first) on `plot`, the default plot when None. The world load falls back to
    /// it when the live save's own home cannot be built (engine/home_plot.rs `assemble_for_boot`).
    pub fn assemble_from(ship: ShipStructure, data_dir: &Path, plot: Option<&str>) -> Result<ShipStructure, String> {
        let (plot_id, kind) = ship.plot_and_kind(plot)?;
        let design = HomeDesign::load(data_dir, &kind)?;
        ship.assemble(design, &plot_id)
    }

    /// `assemble_from` with the player's OWN design (2026-10-04, engine/own_home.rs: a home built
    /// outside the Dev mode is kept in the character's save, not in data/homes/): when `own` is of
    /// the plot's kind and goes on the plot, it is the home; otherwise, or without one, the design
    /// file's is, with a log line saying why the own one was passed over.
    pub fn assemble_from_own(
        ship: ShipStructure,
        data_dir: &Path,
        plot: Option<&str>,
        own: Option<&HomeDesign>,
    ) -> Result<ShipStructure, String> {
        if let Some(design) = own {
            let (plot_id, kind) = ship.plot_and_kind(plot)?;
            if design.kind == kind {
                match ship.clone().assemble(design.clone(), &plot_id) {
                    Ok(assembled) => return Ok(assembled),
                    Err(e) => log::warn!(
                        "your own {kind} home does not go on plot {plot_id} ({e}); the {kind} design file is used instead"
                    ),
                }
            } else {
                log::warn!("your own home is a {} and plot {plot_id} takes a {kind}; the {kind} design file is used", design.kind);
            }
        }
        Self::assemble_from(ship, data_dir, plot)
    }

    /// `plot` (the default plot when None) and the kind of home it takes.
    fn plot_and_kind(&self, plot: Option<&str>) -> Result<(String, String), String> {
        let plot_id = match plot {
            Some(p) => p.to_string(),
            None => self.default_plot_id().ok_or_else(|| "the ship file lists no plots".to_string())?,
        };
        let kind = self
            .plots
            .iter()
            .find(|p| p.id == plot_id)
            .map(|p| p.kind.clone())
            .ok_or_else(|| format!("the ship has no plot '{plot_id}'"))?;
        Ok((plot_id, kind))
    }

    /// The home design this assembled ship carries: the `home` zone's body with the kind and
    /// door point it was assembled from. None for a ship that was not assembled.
    pub fn home_design(&self) -> Option<HomeDesign> {
        let a = self.home.as_ref()?;
        let z = self.zones.iter().find(|z| z.id == HOME_ZONE_ID)?;
        Some(HomeDesign { kind: a.kind.clone(), door: a.door, body: z.body.clone() })
    }

    /// True when corridor `c` is the assembled home's own corridor: the one its plot's door makes.
    /// It is rebuilt from the plot on every load, so the editor offers no delete, no create and
    /// no mouth handle for it; Dev changes it through the plot (`move_plot`, `set_plot_door_zone`).
    pub fn is_plot_door(&self, c: &ShipCorridor) -> bool {
        self.home.is_some() && (c.from_zone == HOME_ZONE_ID || c.to_zone == HOME_ZONE_ID)
    }

    /// After a Dev edit of the plot the home was assembled at, put the home back on it: the home
    /// zone stands at the plot's origin and its corridor is the plot's door again, first in the
    /// list as `assemble` puts it. A no-op on a ship that was not assembled.
    pub fn sync_home_to_plot(&mut self) {
        let Some(plot) = self.home_plot().cloned() else { return };
        if let Some(z) = self.zones.iter_mut().find(|z| z.id == HOME_ZONE_ID) {
            z.origin = plot.origin;
        }
        self.corridors.retain(|c| c.from_zone != HOME_ZONE_ID && c.to_zone != HOME_ZONE_ID);
        self.corridors.insert(0, plot.door.corridor_from(HOME_ZONE_ID));
    }

    /// Move plot `id` to `origin` (the Dev Plots panel). Its door moves with it: the door point is
    /// fixed in the home design, so the corridor's lat shifts by the move along the axis across
    /// the run (z for a door in an east or west wall, x for a north or south one). Moving the plot
    /// the home stands on moves the home, its corridor, its machines and its spawn
    /// (`sync_home_to_plot`; machine offsets are zone-local). False for an unknown plot. The
    /// result may not validate (an overlap, a door that no longer reaches its zone); the panel
    /// shows why and the save refuses to write such a ship.
    pub fn move_plot(&mut self, id: &str, origin: (f32, f32, f32)) -> bool {
        let Some(i) = self.plots.iter().position(|p| p.id == id) else { return false };
        let old = self.plots[i].origin;
        // Which axis the lat lives on: from this ship's home design when the plot takes it (the
        // door point says which wall), else from the corridor the plot's door makes now.
        let lat_on_z = match &self.home {
            Some(a) if a.kind == self.plots[i].kind => {
                let w = self.zones.iter().find(|z| z.id == HOME_ZONE_ID).map_or(0.0, |z| z.body.width);
                Some(a.door.0.abs() < 1e-3 || (a.door.0 - w).abs() < 1e-3)
            }
            _ => self.plot_door_geometry(&self.plots[i]).ok().map(|g| g.axis == CorridorAxis::X),
        };
        let p = &mut self.plots[i];
        match lat_on_z {
            Some(true) => p.door.lat += origin.2 - old.2,
            Some(false) => p.door.lat += origin.0 - old.0,
            None => {}
        }
        p.origin = origin;
        if self.home.as_ref().is_some_and(|a| a.plot == id) {
            self.sync_home_to_plot();
        }
        true
    }

    /// Point plot `id`'s door at another shared zone (the Dev Plots panel). The lat is kept, so
    /// the new zone must span it; the panel shows why when it does not. Refuses the home zone and
    /// an unknown zone or plot.
    pub fn set_plot_door_zone(&mut self, id: &str, zone: &str) -> bool {
        if zone == HOME_ZONE_ID || self.zone_index(zone).is_none() {
            return false;
        }
        let Some(p) = self.plots.iter_mut().find(|p| p.id == id) else { return false };
        p.door.zone = zone.to_string();
        if self.home.as_ref().is_some_and(|a| a.plot == id) {
            self.sync_home_to_plot();
        }
        true
    }

    /// Whether plot `id` would take this ship's home design on the next load, with the reason
    /// when it would not: exactly the refusal `assemble` would give (a door that misses the
    /// design's door, a body that does not fit, an overlap). Ok for a plot of another kind,
    /// which this ship has no design to try. None for an unknown plot or a ship that was not
    /// assembled. For the Plots panel and the save.
    pub fn plot_check(&self, id: &str) -> Option<Result<(), String>> {
        let design = self.home_design()?;
        let plot = self.plots.iter().find(|p| p.id == id)?;
        if plot.kind != design.kind {
            return Some(Ok(()));
        }
        Some(self.ship_file().assemble(design, id).map(|_| ()))
    }

    /// A plot id not yet in use ("p1", "p2", ... the first free number).
    pub fn unique_plot_id(&self) -> String {
        (1..).map(|n| format!("p{n}")).find(|id| !self.plots.iter().any(|p| &p.id == id)).unwrap_or_default()
    }

    /// Add a plot (the Dev Plots panel): a copy of plot `like` with a fresh id, moved past it in
    /// +z by its depth plus `gap` (the way p2 sits beyond p1), its door lat moving with it. The
    /// copy may not validate where it lands (its street may end first); the panel says why.
    /// Returns the new id, or None for an unknown `like`.
    pub fn add_plot_like(&mut self, like: &str, gap: f32) -> Option<String> {
        let mut p = self.plots.iter().find(|p| p.id == like)?.clone();
        let id = self.unique_plot_id();
        p.id = id.clone();
        self.plots.push(p.clone());
        let origin = (p.origin.0, p.origin.1, p.origin.2 + p.size.2 + gap.max(0.0));
        self.move_plot(&id, origin);
        Some(id)
    }

    /// Remove plot `id` (the Dev Plots panel). Refuses the plot the home stands on. Clears
    /// `default_plot` when it named this plot (offline play then uses the first plot), and a
    /// child plot's `parent` link to it. True when removed.
    pub fn remove_plot(&mut self, id: &str) -> bool {
        if self.home.as_ref().is_some_and(|a| a.plot == id) || !self.plots.iter().any(|p| p.id == id) {
            return false;
        }
        self.plots.retain(|p| p.id != id);
        if self.default_plot.as_deref() == Some(id) {
            self.default_plot = None;
        }
        for p in &mut self.plots {
            if p.parent.as_deref() == Some(id) {
                p.parent = None;
            }
        }
        true
    }

    /// Add a district of `type_id` (a zone_types.ron id) at `origin` (ship metres) with `size`;
    /// returns its id, "<type>-<n>" with the first free number, like the shipped "res-1".
    pub fn add_district(&mut self, type_id: &str, origin: (f32, f32, f32), size: (f32, f32, f32)) -> String {
        let id = (1..)
            .map(|n| format!("{type_id}-{n}"))
            .find(|id| !self.districts.iter().any(|d| &d.id == id))
            .unwrap_or_default();
        self.districts.push(Zone { id: id.clone(), type_id: type_id.to_string(), origin, size, label: String::new(), room_type: None });
        id
    }

    /// Remove the district with this id; true when one was removed.
    pub fn remove_district(&mut self, id: &str) -> bool {
        let before = self.districts.len();
        self.districts.retain(|d| d.id != id);
        self.districts.len() != before
    }

    /// The ship file inside this assembled ship: everything but the home zone and its door
    /// corridor. The plot's door record is not rewritten from that corridor: it is the other way
    /// round (`sync_home_to_plot`), and the editor never edits the home's corridor directly
    /// (`is_plot_door`).
    pub fn ship_file(&self) -> ShipStructure {
        let mut s = self.clone();
        if s.home.take().is_some() {
            s.zones.retain(|z| z.id != HOME_ZONE_ID);
            s.corridors.retain(|c| c.from_zone != HOME_ZONE_ID && c.to_zone != HOME_ZONE_ID);
        }
        s
    }

    /// Save an assembled ship back to its two files under `data_dir`: the home design always
    /// (every play mode builds its own home), the ship file only when `ship_scope` (the Dev
    /// mode's ShipStructureEditing), and the home's origin never, because it comes from the
    /// plot. The ship file is written from a COPY with unresolvable corridors pruned and is
    /// refused if it does not validate, so a written file always loads again; the live ship
    /// keeps a broken row on screen (the Corridors panel says why). Ok is a one-line note for
    /// the editor; Err says what was and was not written.
    pub fn save_assembled(&self, data_dir: &Path, ship_scope: bool) -> Result<String, String> {
        let design = self
            .home_design()
            .ok_or_else(|| "this ship was not assembled from a plot, so it has no home file to save".to_string())?;
        let home_path: PathBuf = data_dir.join(HOME_DESIGNS_DIR).join(format!("{}.ron", design.kind));
        design.save(&home_path).map_err(|e| format!("your home was not saved: {e}"))?;
        if !ship_scope {
            return Ok("Saved your home.".to_string());
        }
        let mut ship = self.ship_file();
        let pruned = ship.prune_invalid_corridors();
        if let Err(e) = ship.validate() {
            return Err(format!("Saved your home; the ship was NOT saved: {e}"));
        }
        // Every plot that takes this home design must still take it, or the next load at that
        // plot fails (a Dev move that turns a plot's door away from its street, say).
        for p in ship.plots.iter().filter(|p| p.kind == design.kind) {
            if let Err(e) = ship.clone().assemble(design.clone(), &p.id) {
                return Err(format!("Saved your home; the ship was NOT saved: plot '{}' would not take the {} home: {e}", p.id, design.kind));
            }
        }
        ship.save(&data_dir.join(SHIP_FILE)).map_err(|e| format!("Saved your home; the ship save failed: {e}"))?;
        Ok(if pruned > 0 {
            format!("Saved your home and the ship ({pruned} broken corridor(s) left out of the file).")
        } else {
            "Saved your home and the ship.".to_string()
        })
    }

    /// Write this struct to RON as it stands (the ship-file writer), keeping an existing file's
    /// leading comment header (the v0.526 lesson). `save_assembled` is what the editor calls.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let config = ron::ser::PrettyConfig::default().struct_names(false);
        let body = ron::ser::to_string_pretty(self, config).map_err(|e| e.to_string())?;
        let header = preserved_header(path).unwrap_or_else(|| {
            "// HumanityOS ship structure: the mothership's shared zones (each a fixed outer box +\n\
             // freely placed interior walls, in zone-local metres at a ship `origin`), the corridors\n\
             // between them (each row owns its door mouths: lat = the tube centreline's ship\n\
             // coordinate across the run), the `plots` homes go on, the `default_plot`, and the\n\
             // macro `districts`. Homes are not here: they are data/homes/<kind>.ron designs the\n\
             // loader places on plots. Design doc: docs/design/ship-homes-and-logistics.md.\n\n"
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
        // NEIGHBOURS (increment 2): every other plot drawn as its kind's default design, with its
        // door corridor, render only. Their corridors also open a hole in the shared zone they
        // run to (the street's wall where a neighbour's corridor arrives); those holes are cut
        // in the meshes only, so the wall still stops anyone walking into a neighbour's home.
        let neighbours = crate::ship::neighbours::neighbour_view(self);
        for (zi, z) in self.zones.iter().enumerate() {
            // A home put away is drawn nowhere (`put_home_away`).
            if self.is_away_home(zi) {
                continue;
            }
            let o = z.origin_vec();
            // Corridor apertures cut through this zone's perimeter shell (increment B): the body
            // generates with door-sized holes where corridor tubes meet its box, so a hallway is
            // walkable INTO, not butted against sealed hull. Empty for most zones = the exact
            // pre-B path.
            let mut cuts = self.shell_cuts_for_zone(zi);
            cuts.extend(neighbours.zone_cuts(zi));
            let m = z.body.generate_meshes_with_shell_cuts(&cuts);
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
        // between the two zones' facing perimeter planes -- a floor slab, two side walls, and a
        // lid. The pieces
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
            push_corridor_tube(&mut out, &g, self.zones[g.from_zone_idx].body.shell_material);
            let len = g.end - g.start;
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
                // A corridor is a generated tube, not a zone-named room: no function to join.
                room_type: None,
                label: String::new(),
            });
        }
        // The neighbours' homes and corridors (see above): drawn, never a room, never walked into.
        neighbours.draw_into(&mut out);
        // THE DISTRICTS (ship level since increment 1a; they were sub-zones of the home body):
        // each one's zone_filler.ron contents, in ship metres. Residential districts draw
        // nothing: the homes in them are the plots, each drawn above as its holder's own home or
        // a neighbour's (`HomeStructure::district_fillers`).
        out.material_walls.extend(HomeStructure::district_fillers(&self.districts));
        out
    }
}

/// One corridor TUBE into `out` (increment B): a floor slab, two side walls the full run and the
/// full tube height, and a lid, in the mesh families zone geometry uses (floors /
/// material_walls / ceilings or ceilings_opaque), so the apply path, the render slots and the
/// transparent-glass pass are untouched. `mat` is the shell material it is drawn in. No room and
/// no collision: those are the caller's (`ShipStructure::generate_meshes` registers a ship
/// corridor's walkable bound; a neighbour's corridor gets none, src/ship/neighbours.rs).
pub(crate) fn push_corridor_tube(out: &mut HomesteadMeshes, g: &CorridorGeom, mat: u32) {
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
    let (fv, fi) = floor_quad(Vec3::new(fx, g.floor_y + CORRIDOR_SURFACE_EPS, fz), Vec3::new(sx, 0.0, sz));
    out.floors.push((fv, fi, col, mat));
    // Two side walls, the full run, the full tube height (the SHORTER zone's box height --
    // see CorridorGeom::height). Both merge into one material_walls entry.
    let mut sides: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
    for s in [-1.0f32, 1.0] {
        let (a, b) = match g.axis {
            CorridorAxis::X => (Vec3::new(g.start, 0.0, g.lat + hw * s), Vec3::new(g.end, 0.0, g.lat + hw * s)),
            CorridorAxis::Z => (Vec3::new(g.lat + hw * s, 0.0, g.start), Vec3::new(g.lat + hw * s, 0.0, g.end)),
        };
        merge_shifted(&mut sides, wall_box(a, b, g.floor_y, g.height, CORRIDOR_WALL_THICKNESS), Vec3::ZERO);
    }
    out.material_walls.push((sides.0, sides.1, col));
    // Lid: same span as the floor, dropped 1 cm below the tube top (the same z-fight guard
    // against a zone ceiling at an equal height). A GLASS lid rides the transparent
    // always-visible ceiling pass EXACTLY like a glass zone roof; an opaque lid joins the
    // show-roof-gated opaque pass like a steel zone roof.
    let lid = floor_quad(Vec3::new(fx, g.floor_y + g.height - CORRIDOR_SURFACE_EPS, fz), Vec3::new(sx, 0.0, sz));
    if g.glass_top {
        merge_shifted(&mut out.ceilings, lid, Vec3::ZERO);
    } else {
        merge_shifted(&mut out.ceilings_opaque, lid, Vec3::ZERO);
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
            ..Default::default()
        }
    }

    /// An interior wall carrying one door -- for the desync REGRESSION tests (authored doors must
    /// never influence corridor geometry since the rework).
    fn door_wall(x1: f32, z1: f32, x2: f32, z2: f32) -> crate::ship::home_structure::InteriorWall {
        ron::from_str(&format!(
            "(a: ({x1}, {z1}), b: ({x2}, {z2}), height: 3.0, material: 1, openings: [\
             (kind: Door, at: 1.0, width: 1.5, sill: 0.0, height: 2.1, style: \"swing\", \
             open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: [])])"
        ))
        .expect("door wall literal parses")
    }

    /// Two plain zone boxes, 10 m apart along +X, joined by a corridor at world z = 5. No authored
    /// doors anywhere -- since the rework the corridor OWNS its mouths (1 m wide, 2.1 m tall),
    /// so the fixture needs nothing but the boxes. Home spans z 0..10, commons z 2..10, so the
    /// shared z span is 2..10 and lat 5 sits comfortably inside it.
    fn corridor_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".to_string(),
                    label: "Player Home".to_string(),
                    purpose: "residence".to_string(),
                    origin: (0.0, 0.0, 0.0),
                    body: body(10.0, 10.0, 3.0),
                },
                ShipZone {
                    id: "commons".to_string(),
                    label: "The Commons".to_string(),
                    purpose: "commons".to_string(),
                    origin: (20.0, 0.0, 2.0),
                    body: body(8.0, 8.0, 6.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".to_string(),
                to_zone: "commons".to_string(),
                lat: 5.0,
                width: 3.0,
                door_width: 1.0,
                door_height: 2.1,
                glass_top: false,
            }],
            ..Default::default()
        }
    }

    /// A unique temp path for a round-trip test, deleted with whatever the test made there
    /// when the guard is dropped (BUG-159). Nothing is created.
    fn temp_path(name: &str) -> crate::test_temp::TempPath {
        crate::test_temp::path(&format!("ship_structure_{name}"))
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
    }

    #[test]
    fn empty_and_blank_ids_fail_validation() {
        let none = ShipStructure { zones: Vec::new(), corridors: Vec::new(), ..Default::default() };
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
        // The ship file, the homestead design, and the two assembled on the default plot. The
        // built-in copies (a fresh install, a throwaway relay) assemble just the same.
        let (file, _) = shipped_files();
        assert!(file.zone_index("home").is_none(), "no home lives in the ship file");
        let ship = shipped_ship();
        let home = &ship.zones[ship.home_zone_index()];
        assert_eq!(home.id, "home");
        assert_eq!(ship.home_plot().map(|p| p.id.as_str()), Some("p1"));
        let empty = temp_path("builtin");
        let builtin = ShipStructure::load_and_assemble(&empty, None).expect("the built-in files assemble");
        assert_eq!(ron_of(&builtin.ship_file()), ron_of(&ship.ship_file()), "the built-in ship is the shipped one");
        assert_eq!(ron_of(&builtin.home_design()), ron_of(&ship.home_design()), "a fresh install's own home is the shipped design");
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
        // Required by the rework: every corridor-owned field (lat + door mouth dims) survives a
        // save/load cycle byte-faithfully.
        let ship = corridor_ship();
        let dir = temp_path("corridor_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        ship.save(&path).expect("saves");
        let back = ShipStructure::load(&path).expect("loads back (corridors validate)");
        assert_eq!(back.corridors.len(), 1);
        assert_eq!(back.corridors[0].from_zone, "home");
        assert_eq!(back.corridors[0].to_zone, "commons");
        assert!((back.corridors[0].lat - 5.0).abs() < 1e-6);
        assert!((back.corridors[0].width - 3.0).abs() < 1e-6);
        assert!((back.corridors[0].door_width - 1.0).abs() < 1e-6);
        assert!((back.corridors[0].door_height - 2.1).abs() < 1e-6);
        assert!(!back.corridors[0].glass_top);
    }

    #[test]
    fn a_bad_corridor_no_longer_costs_the_whole_ship_on_load() {
        // The v0.788-migration lesson: validate() rejecting one unresolvable corridor used to
        // return None -> the caller silently reverted the player's ENTIRE ship to the shipped
        // default ("all my saves are gone"). Load now prunes the bad row and keeps every zone.
        let mut ship = two_zone_ship();
        ship.corridors.push(ShipCorridor {
            from_zone: "home".to_string(),
            to_zone: "nonexistent".to_string(),
            lat: 5.0,
            width: 3.0,
            door_width: 2.0,
            door_height: 2.2,
            glass_top: false,
        });
        let dir = temp_path("bad_corridor_load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        // Write WITHOUT save()'s pruning (raw serialize) to simulate a stale/migrated file.
        let body = ron::ser::to_string_pretty(
            &ship,
            ron::ser::PrettyConfig::default().struct_names(false),
        )
        .unwrap();
        std::fs::write(&path, body).unwrap();
        let back = ShipStructure::load(&path).expect("zones survive a bad corridor row");
        assert_eq!(back.zones.len(), 2, "every zone kept");
        assert!(back.corridors.is_empty(), "the unresolvable corridor was pruned");
        assert!(path.exists(), "a recoverable file is never quarantined");
    }

    #[test]
    fn an_unparseable_file_is_quarantined_not_left_to_be_clobbered() {
        // Parse garbage -> load falls back, but the file must be MOVED ASIDE so a later Save
        // (writing the fallback default) can never overwrite the player's only copy.
        let dir = temp_path("quarantine_load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ship_structure.ron");
        std::fs::write(&path, "(zones: [this is not valid RON").unwrap();
        assert!(ShipStructure::load(&path).is_none(), "garbage does not load");
        assert!(!path.exists(), "the bad file was renamed away");
        let quarantined = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("ship_structure.invalid-"));
        assert!(quarantined, "the bad file is preserved under ship_structure.invalid-<ts>.ron");
    }

    #[test]
    fn corridor_door_fields_default_when_omitted() {
        // A row without door_width/door_height gets the serde defaults (2.0 x 2.2 m mouth).
        let ship: ShipStructure = ron::from_str(
            "(zones: [\
             (id: \"home\", body: (width: 10.0, depth: 10.0, height: 3.0)),\
             (id: \"commons\", origin: (20.0, 0.0, 0.0), body: (width: 8.0, depth: 8.0, height: 6.0))],\
             corridors: [(from_zone: \"home\", to_zone: \"commons\", lat: 5.0)])",
        )
        .expect("a defaults-only corridor row parses");
        assert!((ship.corridors[0].width - 3.0).abs() < 1e-6);
        assert!((ship.corridors[0].door_width - 2.0).abs() < 1e-6);
        assert!((ship.corridors[0].door_height - 2.2).abs() < 1e-6);
        assert!(ship.validate().is_ok());
    }

    #[test]
    fn corridor_validation_rejects_bad_references() {
        // Unknown zone.
        let mut ship = corridor_ship();
        ship.corridors[0].to_zone = "nowhere".to_string();
        let e = ship.validate().unwrap_err();
        assert!(e.contains("unknown zone 'nowhere'"), "got: {e}");
        // Same zone on both ends.
        let mut ship = corridor_ship();
        ship.corridors[0].to_zone = "home".to_string();
        let e = ship.validate().unwrap_err();
        assert!(e.contains("itself"), "got: {e}");
        // A door mouth wider than the tube (the tube must enclose its own cut).
        let mut ship = corridor_ship();
        ship.corridors[0].door_width = 5.0;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("exceeds the tube width"), "got: {e}");
    }

    #[test]
    fn corridor_validation_rejects_unbridgeable_zone_pairs() {
        // Overlapping boxes: slide the commons INTO the home footprint -- no clear gap, no run.
        let mut ship = corridor_ship();
        ship.zones[1].origin = (5.0, 0.0, 2.0);
        let e = ship.validate().unwrap_err();
        assert!(e.contains("overlap or touch"), "got: {e}");
        // Diagonal zones: gaps on both axes, but no shared span on the cross axis -- the larger
        // gap picks the run (z here), and the x spans (0..10 vs 20..28) never overlap.
        let mut ship = corridor_ship();
        ship.zones[1].origin = (20.0, 0.0, 40.0);
        let e = ship.validate().unwrap_err();
        assert!(e.contains("do not overlap on the x axis"), "got: {e}");
        // Different deck heights (v1 corridors are level).
        let mut ship = corridor_ship();
        ship.zones[1].origin.1 = 2.5;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("deck heights"), "got: {e}");
    }

    #[test]
    fn a_lat_outside_the_shared_span_errors() {
        // The shared z span is 2..10; a 1 m door needs lat in 2.5..9.5. Just past the top:
        let mut ship = corridor_ship();
        ship.corridors[0].lat = 9.8;
        let e = ship.validate().unwrap_err();
        assert!(e.contains("outside the zones' shared z span"), "got: {e}");
        assert!(e.contains("2.50 to 9.50"), "the error names the valid range, got: {e}");
        // Below the bottom margin too.
        ship.corridors[0].lat = 2.2;
        assert!(ship.validate().is_err());
        // And exactly at the margin is fine.
        ship.corridors[0].lat = 2.5;
        assert!(ship.validate().is_ok(), "lat at the margin boundary is accepted");
    }

    #[test]
    fn corridor_lat_limits_agree_with_the_validator() {
        let mut ship = corridor_ship();
        // Shared z span is 2..10 (home z 0..10, commons z 2..10); the 1 m door needs half a
        // metre of margin each side -> legal lat 2.5..9.5 (the range the validator's error
        // message names in corridor_rejects_a_lat_outside_the_shared_span).
        let (lo, hi) = ship.corridor_lat_limits(&ship.corridors[0]).expect("resolvable pair");
        assert!((lo - 2.5).abs() < 1e-4 && (hi - 9.5).abs() < 1e-4, "got {lo}..{hi}");
        // Both clamp endpoints resolve through the REAL validator -- the whole point of the
        // helper: a viewport drag clamped to [lo, hi] can never strand the row broken.
        ship.corridors[0].lat = lo;
        assert!(ship.corridor_geometry(&ship.corridors[0]).is_ok(), "lat at lo resolves");
        ship.corridors[0].lat = hi;
        assert!(ship.corridor_geometry(&ship.corridors[0]).is_ok(), "lat at hi resolves");
        // Just past either end is rejected -- limits and validator agree on the boundary.
        ship.corridors[0].lat = hi + 0.01;
        assert!(ship.corridor_geometry(&ship.corridors[0]).is_err(), "past hi is rejected");
        ship.corridors[0].lat = lo - 0.01;
        assert!(ship.corridor_geometry(&ship.corridors[0]).is_err(), "past lo is rejected");
        // An unresolvable pair (overlapping boxes) yields no range at all.
        let mut overlapped = corridor_ship();
        overlapped.zones[1].origin = (1.0, 0.0, 1.0);
        assert!(overlapped.corridor_lat_limits(&overlapped.corridors[0]).is_none());
    }

    #[test]
    fn corridor_geometry_spans_the_facing_perimeter_planes() {
        let ship = corridor_ship();
        let g = ship.corridor_geometry(&ship.corridors[0]).expect("valid corridor resolves");
        // Home's facing plane: x = 10 (its +x face); commons' facing plane: x = 20 (its origin).
        assert_eq!(g.end_from, Vec3::new(10.0, 0.0, 5.0));
        assert_eq!(g.end_to, Vec3::new(20.0, 0.0, 5.0));
        assert_eq!(g.axis, CorridorAxis::X);
        assert!((g.start - 10.0).abs() < 1e-4 && (g.end - 20.0).abs() < 1e-4);
        assert!((g.lat - 5.0).abs() < 1e-4);
        assert!((g.height - 3.0).abs() < 1e-4, "the SHORTER zone's height (3 vs 6), got {}", g.height);
        assert_eq!(g.door_from, g.door_to, "both mouths are the corridor's own door");
        assert_eq!(g.door_from, (1.0, 2.1));
    }

    #[test]
    fn authored_door_edits_never_move_the_corridor() {
        // THE desync regression (operator bug 1): the old schema referenced doors by ordinal index
        // into a filtered wall/opening walk, so moving/adding/removing ANY door retargeted every
        // corridor. Since the rework, corridor geometry must be bit-identical no matter what
        // happens to authored doors.
        let ship = corridor_ship();
        let before = ship.corridor_geometry(&ship.corridors[0]).expect("resolves");
        // Add a door-carrying wall at the FRONT of the home's wall list (the exact edit that used
        // to shift every door index) and another at the back of the commons.
        let mut edited = corridor_ship();
        edited.zones[0].body.walls.insert(0, door_wall(2.0, 2.0, 2.0, 8.0));
        edited.zones[1].body.walls.push(door_wall(1.0, 1.0, 7.0, 1.0));
        let after = edited.corridor_geometry(&edited.corridors[0]).expect("still resolves");
        assert_eq!(before, after, "adding doors must not change corridor geometry");
        // Removing every wall (doors and all) changes nothing either.
        let mut stripped = edited;
        stripped.zones[0].body.walls.clear();
        stripped.zones[1].body.walls.clear();
        let after = stripped.corridor_geometry(&stripped.corridors[0]).expect("resolves");
        assert_eq!(before, after, "removing doors must not change corridor geometry");
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
        // Home (zone 0): the tube leaves through its x = w (edge 1) face; the cut is the
        // corridor's own 1 m mouth about lat = 5 -> at = 4.5, door-height 2.1.
        let cuts = ship.shell_cuts_for_zone(0);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].edge, 1);
        assert!((cuts[0].at - 4.5).abs() < 1e-4, "got {}", cuts[0].at);
        assert!((cuts[0].width - 1.0).abs() < 1e-4);
        assert!((cuts[0].height - 2.1).abs() < 1e-4);
        // Commons (zone 1): the tube enters through its x = 0 (edge 3) face. Edge 3 runs from
        // (0, d) to (0, 0), so at = d - (local lat + w/2) = 8 - 3.5 = 4.5.
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
        // prune_invalid_corridors drops a row whose mouth was dragged out of the shared span,
        // keeps the valid shape.
        let mut ship = corridor_ship();
        assert_eq!(ship.prune_invalid_corridors(), 0, "a valid corridor is kept");
        ship.corridors[0].lat = 100.0; // far outside the shared z span (2..10)
        assert_eq!(ship.prune_invalid_corridors(), 1, "the broken row is dropped");
        assert!(ship.corridors.is_empty());
        assert!(ship.validate().is_ok(), "post-prune the ship always validates");
    }

    // ── Shipped-data migration locks (the corridor rework, world coords from the old system) ──

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The ship the game runs: the ship file plus the SHIPPED homestead on its default plot, p1
    /// (never data/homes/homestead.ron, the developer's own home in a checkout).
    fn shipped_ship() -> ShipStructure {
        ShipStructure::load_and_assemble_shipped(&data_dir(), None).expect("the shipped ship assembles at its default plot")
    }

    /// The ship file and the shipped homestead design on their own, as written (no assembly).
    fn shipped_files() -> (ShipStructure, HomeDesign) {
        let ship = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        let design = HomeDesign::built_in("homestead").expect("the homestead design is built in");
        (ship, design)
    }

    #[test]
    fn the_shipped_corridor_resolves_where_the_old_door_pair_did() {
        // Migration lock: pre-rework, the shipped row resolved through home's authored door at
        // world (55, 0, 40) and commons' at (65, 0, 40). The new lat-owned row must produce the
        // SAME tube, or the migration silently moved the hallway. Since increment 1a that row
        // is p1's door, put first by assembly; the Commons-to-street corridor follows it.
        let ship = shipped_ship();
        // The home's door, the Commons to First Street, and since ship homes increment 6 First
        // Street to the cabin block's hall and to the South Commons.
        assert_eq!(ship.corridors.len(), 4);
        assert_eq!((ship.corridors[0].from_zone.as_str(), ship.corridors[0].to_zone.as_str()), ("home", "commons"));
        assert_eq!((ship.corridors[1].from_zone.as_str(), ship.corridors[1].to_zone.as_str()), ("commons", "street-1"));
        assert_eq!((ship.corridors[2].from_zone.as_str(), ship.corridors[2].to_zone.as_str()), ("street-1", "cabins-1"));
        assert_eq!((ship.corridors[3].from_zone.as_str(), ship.corridors[3].to_zone.as_str()), ("street-1", "commons-2"));
        let g = ship.corridor_geometry(&ship.corridors[0]).expect("the shipped corridor resolves");
        assert_eq!(g.axis, CorridorAxis::X);
        assert!((g.start - 55.0).abs() < 1e-4, "got start {}", g.start);
        assert!((g.end - 65.0).abs() < 1e-4, "got end {}", g.end);
        assert!((g.lat - 40.0).abs() < 1e-4, "got lat {}", g.lat);
        assert_eq!(g.end_from, Vec3::new(55.0, 0.0, 40.0));
        assert_eq!(g.end_to, Vec3::new(65.0, 0.0, 40.0));
        assert!((g.height - 3.0).abs() < 1e-4, "the home deck height caps the tube");
        assert!(g.glass_top, "the shipped corridor keeps its glass lid");
    }

    #[test]
    fn the_shipped_corridor_cuts_both_zone_shells_at_the_mouths() {
        // Migration lock, mesh/collision side: the corridor's own 2 m x 2.2 m mouth cuts the home
        // shell on its x = 55 face (edge 1, at = lat - door/2 = 39) and the commons shell on its
        // x = 0 face (edge 3; at = d - (local lat + door/2) = 55 - (20 + 1) = 34). These cuts are
        // now the ONLY walls at the mouths -- the coincident authored door walls were deleted
        // from the RON (operator bug 2: two coplanar walls z-fought and one lacked collision).
        let ship = shipped_ship();
        let home = ship.zone_index("home").expect("home zone exists");
        let cuts = ship.shell_cuts_for_zone(home);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].edge, 1);
        assert!((cuts[0].at - 39.0).abs() < 1e-4, "got {}", cuts[0].at);
        assert!((cuts[0].width - 2.0).abs() < 1e-4);
        assert!((cuts[0].height - 2.2).abs() < 1e-4);
        let commons = ship.zone_index("commons").expect("commons zone exists");
        let cuts = ship.shell_cuts_for_zone(commons);
        assert_eq!(cuts.len(), 2, "the home's corridor and the street's");
        let west = cuts.iter().find(|c| c.edge == 3).expect("the home corridor's mouth on the west face");
        assert!((west.at - 34.0).abs() < 1e-4, "got {}", west.at);
        // The street corridor leaves through the Commons' south face (z = 75, edge 2, which winds
        // -x from x = w): at = w - (local lat + door/2) = 34 - (5 + 1) = 28.
        let south = cuts.iter().find(|c| c.edge == 2).expect("the street corridor's mouth on the south face");
        assert!((south.at - 28.0).abs() < 1e-4, "got {}", south.at);
    }

    /// v0.789 regression (operator: "there's still a wall in the corridor"):
    /// an INTERVENING zone whose perimeter crosses the tube's path gets the
    /// same door-sized cut the end mouths get. Fixture mirrors the live ship:
    /// a big region zone (his 120x200 Residential) overlapping the run between
    /// home and commons, its west face at x = 7 crossing the 10..20 gap...
    /// here the region spans x 12..40 so only its WEST face (x = 12) sits
    /// inside the tube span (10..20) -- exactly one cut, on edge 3, at lat.
    #[test]
    fn an_intervening_zone_shell_gets_cut_where_the_tube_crosses_it() {
        let mut ship = corridor_ship();
        ship.zones.push(ShipZone {
            id: "region".to_string(),
            label: "Residential".to_string(),
            purpose: "residential".to_string(),
            origin: (12.0, 0.0, 0.0),
            body: body(28.0, 30.0, 4.0),
        });
        let region = ship.zone_index("region").expect("region zone exists");
        let cuts = ship.shell_cuts_for_zone(region);
        assert_eq!(cuts.len(), 1, "exactly the west-face crossing is cut");
        assert_eq!(cuts[0].edge, 3, "west face (x = origin.x) is edge 3");
        // Edge 3 winds -z from z = d: at = d - (local lat + door/2) = 30 - (5 + 0.5).
        assert!((cuts[0].at - 24.5).abs() < 1e-4, "got {}", cuts[0].at);
        assert!((cuts[0].width - 1.0).abs() < 1e-4, "door-sized, not tube-sized");

        // A zone the tube never touches (lat outside its span) cuts nothing.
        ship.zones.push(ShipZone {
            id: "aside".to_string(),
            label: "Aside".to_string(),
            purpose: "storage".to_string(),
            origin: (12.0, 0.0, 20.0),
            body: body(6.0, 6.0, 3.0),
        });
        let aside = ship.zone_index("aside").expect("aside zone exists");
        assert!(ship.shell_cuts_for_zone(aside).is_empty());
    }

    /// Corridor door panels (v0.795): every valid corridor exposes its two END mouths, in world
    /// space, on the zones' facing perimeter planes -- where `corridor_geometry` puts end_from /
    /// end_to and where `shell_cuts_for_zone` opens the apertures.
    #[test]
    fn corridor_mouths_cover_both_tube_ends() {
        let ship = corridor_ship();
        let mouths = ship.corridor_mouths();
        assert_eq!(mouths.len(), 2, "one mouth per tube end");
        for (m, plane) in mouths.iter().zip([10.0f32, 20.0]) {
            assert_eq!(m.axis, CorridorAxis::X);
            assert!((m.plane - plane).abs() < 1e-4, "mouth on the facing shell, got {}", m.plane);
            assert!((m.lat - 5.0).abs() < 1e-4, "aperture centred on the row's lat");
            assert!(m.floor_y.abs() < 1e-4, "sill on the shared deck");
            assert_eq!(m.door, (1.0, 2.1), "the corridor's own door size");
        }
    }

    /// An INTERVENING zone's perimeter crossing (the v0.789 shell-cut case) gets a mouth too, with
    /// its height clamped to that zone's box exactly like the cut; a coplanar second crossing is
    /// deduped (one door pair per world aperture); a zone the tube misses adds nothing.
    #[test]
    fn corridor_mouths_include_intervening_crossings_clamped_and_deduped() {
        let mut ship = corridor_ship();
        // A SHORT region zone whose west face (x = 12) crosses the 10..20 run: its mouth's door
        // height clamps to the 1.8 m box (the 2.1 m door would poke above its shell cut).
        ship.zones.push(zone("region", (12.0, 0.0, 0.0), 28.0, 30.0, 1.8));
        let mouths = ship.corridor_mouths();
        assert_eq!(mouths.len(), 3, "two ends + one crossing");
        let crossing = &mouths[2];
        assert!((crossing.plane - 12.0).abs() < 1e-4, "west face crossing, got {}", crossing.plane);
        assert!((crossing.door.1 - 1.8).abs() < 1e-4, "height clamped to the crossing zone's box");
        assert!((crossing.door.0 - 1.0).abs() < 1e-4, "width stays the corridor's own door");
        // A second zone with a COPLANAR west face at x = 12: still one crossing mouth there
        // (dedup), or two door pairs would z-fight in the same aperture. Its east face lands at
        // x = 20 -- the commons end-mouth plane -- which the strictly-inside test excludes (that
        // aperture already has its end doors).
        ship.zones.push(zone("annex", (12.0, 0.0, 0.0), 8.0, 30.0, 3.0));
        assert_eq!(ship.corridor_mouths().len(), 3, "coplanar + end-coincident planes add nothing");
        // A zone whose span never reaches the tube's lat adds nothing.
        ship.zones.push(zone("aside", (12.0, 0.0, 20.0), 6.0, 6.0, 3.0));
        assert_eq!(ship.corridor_mouths().len(), 3, "an untouched zone contributes no mouth");
    }

    /// A broken corridor row (mouth dragged out of the shared span) gets NO mouths -- exactly the
    /// rows mesh + collision skip, so a door can never float where no tube resolves.
    #[test]
    fn a_broken_corridor_row_gets_no_mouths() {
        let mut ship = corridor_ship();
        ship.corridors[0].lat = 100.0; // far outside the zones' shared z span
        assert!(ship.corridor_mouths().is_empty());
    }

    /// Everything that belongs to the home, in ship metres: its rooms (id, centre, size), its
    /// collision segments, its machines (the home rows of data/machines/home.ron, placed the way
    /// the engine places them) and the spawn point. The moved-plot test compares two of these.
    struct HomeParts {
        rooms: Vec<(String, Vec3, Vec3)>,
        segments: Vec<((f32, f32), (f32, f32))>,
        machines: Vec<(String, (f32, f32, f32))>,
        spawn: Option<Vec3>,
    }

    fn home_parts(ship: &ShipStructure) -> HomeParts {
        let hi = ship.home_zone_index();
        let home = &ship.zones[hi];
        let o = home.origin_vec();
        // The home zone keeps its raw room ids; other zones prefix "<zone>:" and corridors are
        // "corridor_<n>", so the home's rooms are the ones with neither.
        let rooms = ship
            .generate_meshes()
            .room_info
            .into_iter()
            .filter(|r| !r.id.contains(':') && !r.id.starts_with("corridor_"))
            .map(|r| (r.id, r.center, r.dimensions))
            .collect();
        let segments = crate::ship::wall_collision::wall_segments_with_shell_cuts(
            &home.body,
            &ship.shell_cuts_for_zone(hi),
        )
        .into_iter()
        .map(|s| ((s.a.0 + o.x, s.a.1 + o.z), (s.b.0 + o.x, s.b.1 + o.z)))
        .collect();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let machines_file = crate::machines::MachineHome::load(&root.join("data").join("machines").join("home.ron"))
            .expect("home.ron parses");
        let home_ids: std::collections::HashSet<String> = machines_file
            .all_instances()
            .into_iter()
            .filter(|i| i.zone == "home")
            .map(|i| i.id)
            .collect();
        let machines = machines_file
            .placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()))
            .into_iter()
            .filter(|p| home_ids.contains(&p.id))
            .map(|p| (p.id, p.pos))
            .collect();
        HomeParts { rooms, segments, machines, spawn: ship.home_spawn_world() }
    }

    /// Every way `after` fails to be `before` moved by (dx, 0, dz), as plain sentences (empty when
    /// the whole home moved with its plot).
    fn not_moved_by(before: &HomeParts, after: &HomeParts, dx: f32, dz: f32) -> Vec<String> {
        let d = Vec3::new(dx, 0.0, dz);
        let near = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 1e-3;
        let mut out = Vec::new();
        if before.rooms.len() != after.rooms.len() {
            out.push(format!("room count changed: {} -> {}", before.rooms.len(), after.rooms.len()));
        }
        let rooms_off = before
            .rooms
            .iter()
            .filter(|(id, c, s)| !after.rooms.iter().any(|(i2, c2, s2)| i2 == id && near(*c + d, *c2) && near(*s, *s2)))
            .count();
        if rooms_off > 0 {
            out.push(format!("{rooms_off} of {} rooms did not move with the plot", before.rooms.len()));
        }
        if before.segments.len() != after.segments.len() {
            out.push(format!("segment count changed: {} -> {}", before.segments.len(), after.segments.len()));
        }
        let segs_off = before
            .segments
            .iter()
            .zip(&after.segments)
            .filter(|((a, b), (a2, b2))| {
                (a.0 + dx - a2.0).abs() > 1e-3
                    || (a.1 + dz - a2.1).abs() > 1e-3
                    || (b.0 + dx - b2.0).abs() > 1e-3
                    || (b.1 + dz - b2.1).abs() > 1e-3
            })
            .count();
        if segs_off > 0 {
            out.push(format!("{segs_off} of {} collision segments did not move with the plot", before.segments.len()));
        }
        let machines_off: Vec<&str> = before
            .machines
            .iter()
            .filter(|(id, p)| {
                !after.machines.iter().any(|(i2, p2)| {
                    i2 == id && near(Vec3::new(p.0, p.1, p.2) + d, Vec3::new(p2.0, p2.1, p2.2))
                })
            })
            .map(|(id, _)| id.as_str())
            .collect();
        if !machines_off.is_empty() {
            out.push(format!(
                "{} of {} home machines did not move with the plot (first: {:?})",
                machines_off.len(),
                before.machines.len(),
                &machines_off[..machines_off.len().min(3)]
            ));
        }
        match (before.spawn, after.spawn) {
            (Some(a), Some(b)) if near(a + d, b) => {}
            (a, b) => out.push(format!("the spawn did not move with the plot: {a:?} -> {b:?}")),
        }
        out
    }

    /// The shipped homestead assembled on a copy of p1 moved by (dx, 0, dz). The plot's door lat
    /// moves with it: the door is in the east wall, so its corridor runs along X and the lat is
    /// a z.
    fn assembled_on_p1_moved_by(dx: f32, dz: f32) -> ShipStructure {
        let (mut ship, design) = shipped_files();
        let p1 = ship.plots.iter_mut().find(|p| p.id == "p1").expect("the ship has p1");
        p1.origin.0 += dx;
        p1.origin.2 += dz;
        p1.door.lat += dz;
        ship.assemble(design, "p1").expect("the moved plot assembles")
    }

    /// Moving a home's plot by (dx, 0, dz) moves every room, collision segment, machine and the
    /// spawn by exactly that, and nothing of the home stays behind (increment 1a,
    /// docs/design/ship-homes-and-logistics.md section 7). Two moves: the real second plot p2
    /// (p1 moved 99 m south, its door on street-1 instead of the Commons), and p1 moved 100 m
    /// west (its corridor to the Commons just grows longer).
    ///
    /// SEEN RED FIRST, on the code and data from before the split, with "move the plot" meaning
    /// the home zone's origin plus its corridor's lat moved by (0, 0, 10):
    ///   259 of 259 home machines did not move with the plot (first: ["shelf_1", "side_table_2", "rug_3"])
    ///   the spawn did not move with the plot: Some(Vec3(53.5, 1.7, 40.5)) -> Some(Vec3(53.5, 1.7, 40.5))
    /// (the 23 rooms and 69 collision segments already moved). Machine offsets were absolute
    /// ship positions clamped into the zone, and the spawn ignored the zone's origin.
    #[test]
    fn moving_the_plot_moves_everything_in_the_home() {
        let before = home_parts(&shipped_ship());
        // 282 since the homestead's upper floor (2026-10-20): 23 more for the dining room, the
        // second bedroom, the boot room and the shower room.
        assert_eq!(before.machines.len(), 282, "every home machine is placed");
        let p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).expect("the homestead assembles at p2");
        let mut failures = not_moved_by(&before, &home_parts(&p2), 0.0, 99.0);
        failures.extend(not_moved_by(&before, &home_parts(&assembled_on_p1_moved_by(-100.0, 0.0)), -100.0, 0.0));
        assert!(failures.is_empty(), "moving the plot left things behind:\n  {}", failures.join("\n  "));
    }

    /// Today's ship, pinned from the shipped data BEFORE the split (2026-10-03): every room the
    /// whole ship detected, as (id, centre, size). 30 are the home's, 10 the Commons', 1 the
    /// home's corridor. The wall-segment counts are below. Re-pinned 2026-10-20 when the homestead
    /// gained its upper floor (ship homes increment 7): its 23 rooms became 30 (the quiet wing's
    /// five at y 4.5 upstairs with the upper hall, the back corridor and the lift lobby; a dining
    /// room, a second bedroom, a shower room and a boot room downstairs; the greenhouse 6 m tall),
    /// and the Commons' rooms and the corridor did not move.
    const ROOMS_BEFORE_THE_SPLIT: &[(&str, [f32; 3], [f32; 3])] = &[
        ("commons:room_1", [82.0, 4.0, 47.5], [34.0, 8.0, 55.0]),
        ("commons:room_10", [71.0, 4.0, 46.0], [3.0, 8.0, 5.0]),
        ("commons:room_2", [71.5, 4.0, 31.0], [4.0, 8.0, 3.0]),
        ("commons:room_3", [71.5, 4.0, 36.0], [4.0, 8.0, 5.0]),
        ("commons:room_4", [75.5, 4.0, 34.5], [2.0, 8.0, 2.0]),
        ("commons:room_5", [79.0, 4.0, 36.0], [3.0, 8.0, 5.0]),
        ("commons:room_6", [75.5, 4.0, 37.5], [2.0, 8.0, 2.0]),
        ("commons:room_7", [75.0, 4.0, 40.0], [11.0, 8.0, 1.0]),
        ("commons:room_8", [71.0, 4.0, 42.0], [3.0, 8.0, 1.0]),
        ("commons:room_9", [77.0, 4.0, 45.0], [7.0, 8.0, 7.0]),
        ("console-room", [53.75, 1.5, 47.0], [2.5, 3.0, 7.0]),
        ("corridor_0", [60.0, 1.5, 40.0], [10.0, 3.0, 3.0]),
        ("room-aquaponics", [4.75, 1.5, 67.0], [9.5, 3.0, 11.0]),
        ("room-back-corridor", [33.0, 4.5, 35.25], [1.0, 3.0, 9.5]),
        ("room-barn", [47.75, 1.5, 81.25], [14.5, 3.0, 15.5]),
        ("room-bathroom", [47.5, 4.5, 32.0], [4.0, 3.0, 3.0]),
        ("room-bedroom", [38.0, 4.5, 34.0], [7.0, 3.0, 7.0]),
        ("room-bedroom-2", [46.0, 1.5, 34.0], [7.0, 3.0, 7.0]),
        ("room-boot", [52.75, 1.5, 36.0], [4.5, 3.0, 3.0]),
        ("room-common", [47.0, 1.5, 47.0], [9.0, 3.0, 7.0]),
        ("room-court", [44.75, 1.5, 26.0], [20.5, 3.0, 7.0]),
        ("room-dining", [38.0, 1.5, 34.0], [7.0, 3.0, 7.0]),
        ("room-dressing", [43.5, 4.5, 34.0], [2.0, 3.0, 7.0]),
        ("room-entry", [53.75, 1.5, 40.5], [2.5, 3.0, 4.0]),
        ("room-fields", [19.75, 1.5, 81.25], [39.5, 3.0, 15.5]),
        ("room-forge", [16.0, 1.5, 40.5], [7.0, 3.0, 20.0]),
        ("room-greenhouse", [32.75, 3.0, 62.0], [44.5, 6.0, 21.0]),
        ("room-hall", [43.0, 1.5, 40.5], [17.0, 3.0, 4.0]),
        ("room-kitchen", [38.0, 1.5, 48.5], [7.0, 3.0, 4.0]),
        ("room-lift-lobby", [32.0, 4.5, 28.25], [3.0, 3.0, 2.5]),
        ("room-mushroom", [4.75, 1.5, 56.0], [9.5, 3.0, 9.0]),
        ("room-pantry", [38.0, 1.5, 44.5], [7.0, 3.0, 2.0]),
        ("room-plant", [5.75, 1.5, 40.5], [11.5, 3.0, 20.0]),
        ("room-power", [9.75, 1.5, 10.75], [19.5, 3.0, 21.5]),
        ("room-serviceway", [16.75, 1.5, 26.0], [33.5, 3.0, 7.0]),
        ("room-shower", [52.75, 1.5, 32.0], [4.5, 3.0, 3.0]),
        ("room-study", [52.75, 4.5, 34.0], [4.5, 3.0, 7.0]),
        ("room-upper-hall", [44.75, 4.5, 39.25], [20.5, 3.0, 1.5]),
        ("room-vehicle", [37.75, 1.5, 10.75], [34.5, 3.0, 21.5]),
        ("room-wetroom", [47.5, 4.5, 36.0], [4.0, 3.0, 3.0]),
        ("room-workshop", [27.0, 1.5, 40.5], [13.0, 3.0, 20.0]),
    ];
    /// Whole-ship wall segments before the split (collision 154, sight 174), and the home's
    /// own share (101). Before the homestead's upper floor (2026-10-20): 122, 135 and 69.
    const WALL_SEGMENTS_BEFORE: usize = 154;
    const SIGHT_SEGMENTS_BEFORE: usize = 174;
    const HOME_SEGMENTS_BEFORE: usize = 101;

    /// Assembling at p1 reproduces today's ship. Everything that existed before the split (the
    /// assembled ship without street-1 and its corridor, both new in 1a, and without the cabin
    /// block, the South Commons and the recreation room of increment 6) has the same 41 rooms,
    /// each with the same id, centre and size, and the same 154 collision and 174 sight
    /// segments; the home alone keeps its 30 rooms and 101 segments (23 and 69, and 34, 122 and
    /// 135 for the ship, before the homestead's upper floor of 2026-10-20). Then the whole assembled ship
    /// is pinned too: street-1 adds one room (its open box) and its corridor a walkable bound
    /// (36 rooms); its four walls, the cut its corridor makes in the street's north wall and in
    /// the Commons' south wall, and the corridor's two side rails add 8 segments (130).
    #[test]
    fn assembling_at_p1_reproduces_todays_rooms_and_segments() {
        use crate::ship::wall_collision::{ship_sight_segments, ship_wall_segments};
        let ship = shipped_ship();
        let home = home_parts(&ship);
        // 30 since the homestead's upper floor (2026-10-20): the ground floor's 22 (the quiet wing
        // went up, a dining room, a second bedroom, a shower room and a boot room came) and the
        // upper floor's 8.
        assert_eq!(home.rooms.len(), 30, "the home's own rooms");
        assert_eq!(home.segments.len(), HOME_SEGMENTS_BEFORE, "the home's own collision segments");

        // The ship before the split: without street-1 and its corridors (1a), and without what
        // ship homes increment 6 (lane 1A) added: the cabin block's hall and the South Commons
        // (whose corridors run from street-1), the sixteen cabin plots, and the Commons'
        // recreation room.
        let mut before_shape = ship.clone();
        before_shape.zones.retain(|z| !["street-1", "cabins-1", "commons-2"].contains(&z.id.as_str()));
        before_shape.corridors.retain(|c| c.from_zone != "street-1" && c.to_zone != "street-1");
        before_shape.plots.retain(|p| p.kind != "cabin");
        let ci = before_shape.zone_index("commons").expect("the Commons");
        before_shape.zones[ci].body.zones.retain(|v| v.id != "recreation");
        let rooms = before_shape.generate_meshes().room_info;
        assert_eq!(rooms.len(), ROOMS_BEFORE_THE_SPLIT.len(), "today's room count");
        for (id, c, s) in ROOMS_BEFORE_THE_SPLIT {
            let r = rooms
                .iter()
                .find(|r| r.id == *id)
                .unwrap_or_else(|| panic!("room '{id}' from before the split is gone"));
            assert!(
                (r.center - Vec3::from(*c)).abs().max_element() < 1e-3
                    && (r.dimensions - Vec3::from(*s)).abs().max_element() < 1e-3,
                "room '{id}' moved or changed size: {:?} {:?}",
                r.center,
                r.dimensions
            );
        }
        assert_eq!(ship_wall_segments(&before_shape).len(), WALL_SEGMENTS_BEFORE, "today's collision segments");
        // Since the increment 2 review (finding 8) the sight lines also see the neighbours' homes:
        // here p2's, drawn without its corridor (street-1, its door zone, is not in this shape), so
        // its share is one shipped homestead's own sight segments with no door cut. Counted from
        // the plots and the shipped design, NOT from the neighbour view under test, which would
        // pass with no neighbour segments at all (the final review of increment 2). Seen red
        // 2026-10-04 with `NeighbourView::segments` returning nothing: "today's sight segments,
        // and one bare homestead shell per neighbour plot (1 x 74): left 135, right 209".
        let neighbour_plots = before_shape.plots.iter().filter(|p| Some(p.id.as_str()) != before_shape.home_plot().map(|h| h.id.as_str())).count();
        assert_eq!(neighbour_plots, before_shape.plots.len() - 1, "every plot but p1 is a neighbour's (p2 alone until the twelve plots of 2026-10-04)");
        let bare_shell = crate::ship::wall_collision::sight_segments_with_shell_cuts(
            &HomeDesign::built_in("homestead").expect("the homestead is built in").body,
            &[],
        )
        .len();
        assert!(bare_shell > 4, "a homestead's sight segments: its shell and its rooms' walls ({bare_shell})");
        assert_eq!(
            ship_sight_segments(&before_shape).len(),
            SIGHT_SEGMENTS_BEFORE + neighbour_plots * bare_shell,
            "today's sight segments, and one bare homestead shell per neighbour plot ({neighbour_plots} x {bare_shell})"
        );

        // The whole ship: street-1's room and its corridor's (36 rooms, 130 segments after 1a);
        // since ship homes increment 6 (lane 1A) five rooms more, the cabin block's hall and its
        // washroom (its wall with a door makes two), the South Commons, and the two corridors
        // from First Street to them (41); and 18 segments more, the hall's and the South
        // Commons' walls, the washroom's wall either side of its door, the cuts the two
        // corridors make in their walls and in First Street's east wall, and their rails (148).
        let all = ship.generate_meshes().room_info;
        assert_eq!(all.len(), 48, "the whole ship's rooms");
        assert!(all.iter().any(|r| r.id == "cabins-1:washroom-1"), "the cabin block's washroom is a room");
        // 48 rooms and 180 segments since the homestead's upper floor (2026-10-20); 41 and 148 before.
        assert_eq!(ship_wall_segments(&ship).len(), 180, "the whole ship's collision segments");
        // The home's corridor keeps its id in the whole ship too: assembly puts it first.
        let c0 = all.iter().find(|r| r.id == "corridor_0").expect("corridor_0");
        assert!((c0.center - Vec3::new(60.0, 1.5, 40.0)).abs().max_element() < 1e-3, "corridor_0 is the home's corridor, got {:?}", c0.center);
    }

    /// A tiny ship for the overlap table: a 10 x 100 m Commons at x 20..30, and plots west of
    /// it whose doors run east to it.
    fn plot(id: &str, z: f32, depth: f32, lat: f32) -> Plot {
        Plot {
            id: id.to_string(),
            parent: None,
            kind: "test".to_string(),
            origin: (0.0, 0.0, z),
            size: (10.0, 3.0, depth),
            door: PlotDoor {
                zone: "commons".to_string(),
                lat,
                width: 3.0,
                door_width: 2.0,
                door_height: 2.2,
                glass_top: true,
            },
            neighbourhood: String::new(),
        }
    }

    fn overlap_fixture(plots: Vec<Plot>, extra_zones: Vec<ShipZone>) -> ShipStructure {
        let mut zones = vec![zone("commons", (20.0, 0.0, 0.0), 10.0, 100.0, 4.0)];
        zones.extend(extra_zones);
        ShipStructure { zones, plots, ..Default::default() }
    }

    /// NESTED PLOTS are never each other's neighbours (increment 2 review, finding 7). `validate`
    /// lets a child plot sit inside its parent (a cabin in its block, "Cabins and Apartments,
    /// later"): with the home on the child, the block drawn as a neighbour put its default shell
    /// over the home; with the home on the block, every child was drawn inside it. A plot beside
    /// them stays a neighbour either way, and while the home is put away every plot is one. Seen
    /// red 2026-10-04 on c98c5465b (only the own plot left out): "on the cabin c1, its block is
    /// drawn as a neighbour: [\"block\", \"g1\", \"p9\"]".
    #[test]
    fn a_child_plots_parent_is_not_drawn_over_the_home() {
        let child = Plot { parent: Some("block".into()), ..plot("c1", 30.0, 5.0, 32.5) };
        let grandchild = Plot { parent: Some("c1".into()), ..plot("g1", 31.0, 2.0, 32.0) };
        let mut ship = overlap_fixture(vec![plot("block", 30.0, 20.0, 40.0), child, grandchild, plot("p9", 60.0, 10.0, 65.0)], vec![]);
        let on = |ship: &mut ShipStructure, id: &str| {
            ship.home = Some(HomeAssembly { plot: id.into(), kind: "test".into(), door: (10.0, 2.5), away: false });
            ship.neighbour_plots().map(|p| p.id.clone()).collect::<Vec<_>>()
        };
        assert_eq!(on(&mut ship, "c1"), ["p9"], "on the cabin c1, its block is drawn as a neighbour: {:?}", on(&mut ship, "c1"));
        assert_eq!(on(&mut ship, "block"), ["p9"], "on the block, its cabins are drawn inside it");
        assert_eq!(on(&mut ship, "g1"), ["p9"], "every ancestor, not only the parent");
        assert_eq!(on(&mut ship, "p9"), ["block", "c1", "g1"], "a plot beside them sees them all");
        ship.home.as_mut().unwrap().away = true;
        assert_eq!(ship.neighbour_plots().count(), 4, "a home put away: every plot is a neighbour's");
    }

    /// validate() refuses overlapping plots, zones and corridor tubes, and lets touching ones,
    /// a child plot inside its parent, and districts through. Each row names what must be
    /// refused (a fragment of the error) or None for a ship that must validate.
    /// Red check, run: making `validate_overlaps` return Ok(()) fails every refusing row.
    #[test]
    fn overlapping_plots_zones_and_tubes_are_refused() {
        let child = |id: &str, parent: &str, z: f32, depth: f32, lat: f32| Plot { parent: Some(parent.to_string()), ..plot(id, z, depth, lat) };
        let mut district_ship = overlap_fixture(vec![plot("p1", 0.0, 10.0, 5.0)], vec![]);
        district_ship.districts.push(Zone {
            id: "res".into(),
            type_id: "residential".into(),
            origin: (-50.0, 0.0, -50.0),
            size: (200.0, 4.0, 200.0),
            label: String::new(),
            room_type: None,
        });
        let cases: Vec<(&str, ShipStructure, Option<&str>)> = vec![
            ("two plots that touch", overlap_fixture(vec![plot("p1", 0.0, 10.0, 5.0), plot("p2", 10.0, 10.0, 15.0)], vec![]), None),
            ("two plots that overlap", overlap_fixture(vec![plot("p1", 0.0, 10.0, 5.0), plot("p2", 5.0, 10.0, 12.0)], vec![]), Some("plot 'p1' overlaps plot 'p2'")),
            ("a plot on a zone", overlap_fixture(vec![plot("p1", 50.0, 10.0, 55.0)], vec![zone("bay", (0.0, 0.0, 45.0), 10.0, 10.0, 3.0)]), Some("plot 'p1' overlaps zone 'bay'")),
            ("two zones that overlap", overlap_fixture(vec![], vec![zone("bay", (0.0, 0.0, 0.0), 22.0, 10.0, 3.0)]), Some("zone 'commons' overlaps zone 'bay'")),
            ("two zones that touch", overlap_fixture(vec![], vec![zone("bay", (10.0, 0.0, 0.0), 10.0, 10.0, 3.0)]), None),
            ("a zone across a plot's corridor", overlap_fixture(vec![plot("p1", 0.0, 10.0, 5.0)], vec![zone("kiosk", (12.0, 0.0, 4.0), 4.0, 2.0, 3.0)]), Some("zone 'kiosk' overlaps the corridor from plot 'p1' to commons")),
            ("two corridors that overlap", overlap_fixture(vec![plot("p1", 0.0, 10.0, 9.0), plot("p2", 10.0, 10.0, 11.0)], vec![]), Some("the corridor from plot 'p1' to commons overlaps the corridor from plot 'p2' to commons")),
            ("a child plot inside its parent", overlap_fixture(vec![plot("block", 30.0, 20.0, 40.0), child("c1", "block", 30.0, 5.0, 32.5)], vec![]), None),
            ("a child plot sticking out of its parent", overlap_fixture(vec![plot("block", 30.0, 20.0, 40.0), child("c1", "block", 48.0, 5.0, 50.5)], vec![]), Some("plot 'c1' is not inside its parent 'block'")),
            ("a district over everything", district_ship, None),
        ];
        let mut wrong = Vec::new();
        for (name, ship, want) in &cases {
            match (ship.validate(), want) {
                (Ok(()), None) => {}
                (Err(e), Some(frag)) if e.contains(frag) => {}
                (got, want) => wrong.push(format!("{name}: wanted {want:?}, got {got:?}")),
            }
        }
        assert!(wrong.is_empty(), "overlap table:\n  {}", wrong.join("\n  "));
    }

    /// Assembly refuses what does not fit, and splitting an assembled ship gives back exactly
    /// the two files it came from (so a save writes nothing it did not read).
    #[test]
    fn assembly_refuses_what_does_not_fit_and_splits_back_into_its_files() {
        let (ship, design) = shipped_files();
        assert!(ship.zone_index(HOME_ZONE_ID).is_none(), "the ship file holds no home");
        // p1 to p12 (the twelve plots along First Street, 2026-10-04), and since ship homes
        // increment 6 the sixteen cabins c1 to c16.
        assert_eq!(ship.plots.len(), 28, "p1 to p12 and c1 to c16");
        assert_eq!(ship.default_plot_id().as_deref(), Some("p1"));
        let refuse = |s: ShipStructure, d: HomeDesign, plot: &str, frag: &str| {
            let e = s.assemble(d, plot).expect_err(frag);
            assert!(e.contains(frag), "wanted '{frag}', got: {e}");
        };
        refuse(ship.clone(), design.clone(), "p99", "no plot 'p99'");
        let mut cabin = design.clone();
        cabin.kind = "cabin".to_string();
        refuse(ship.clone(), cabin, "p1", "takes a homestead home, not a cabin");
        let mut moved_door = design.clone();
        moved_door.door = (55.0, 30.0);
        refuse(ship.clone(), moved_door, "p2", "does not meet");
        let mut inner_door = design.clone();
        inner_door.door = (20.0, 30.0);
        refuse(ship.clone(), inner_door, "p1", "is not on its");
        let mut big = design.clone();
        big.body.width = 60.0;
        refuse(ship.clone(), big, "p1", "does not fit plot 'p1'");
        // The right lat through the wrong wall (the critic's case): a plot EAST of street-1 at
        // (80, 0, 300), its door lat 340 on the street. The lat meets the design's door (local
        // z 40), but the door is in the home's east wall and the corridor would cut the west
        // one. Red check, run: without the door-face check this plot assembles (nothing
        // overlaps there), and this refusal fails.
        // (Named "east": p3 is a real plot since the twelve plots along First Street. At z 300,
        // not 99: since ship homes increment 6 the cabin block stands east of the street at z 88
        // to 136.)
        let mut east = ship.clone();
        let mut across = east.plots.iter().find(|p| p.id == "p2").cloned().expect("p2");
        across.id = "east".to_string();
        across.origin = (80.0, 0.0, 300.0);
        across.door.lat = 340.0;
        east.plots.push(across);
        refuse(east, design.clone(), "east", "not through the homestead design's door at (135.0, 340.0)");
        let assembled = ship.clone().assemble(design.clone(), "p1").expect("assembles");
        refuse(assembled.clone(), design.clone(), "p2", "zone named 'home'");

        assert_eq!(ron_of(&assembled.ship_file()), ron_of(&ship), "the ship file comes back unchanged");
        assert_eq!(ron_of(&assembled.home_design().expect("assembled")), ron_of(&design), "the home design comes back unchanged");
        assert_eq!(assembled.home_plot().map(|p| p.id.as_str()), Some("p1"));
        let home = &assembled.zones[assembled.home_zone_index()];
        assert_eq!((home.id.as_str(), home.origin), ("home", (0.0, 0.0, 0.0)), "the home sits at p1's origin");
    }

    /// RON text of either file type, for the round-trip comparisons above.
    fn ron_of<T: Serialize>(v: &T) -> String {
        ron::ser::to_string(v).expect("serializes")
    }

    /// A copy of the shipped data files in a temp dir, for the save tests: the ship file, and the
    /// player's own home as a fresh install starts it (the shipped design under the own-home
    /// header, `get_embedded`), never the checkout's data/homes/homestead.ron, the developer's
    /// own home (with one wall added there by an editor Save, `save_assembled_never_writes_the_
    /// shipped_home_default` failed "the player's own home has the new wall: left 29, right 28").
    fn temp_data_dir(name: &str) -> crate::test_temp::TempPath {
        let dir = temp_path(name);
        let ship = dir.join(SHIP_FILE);
        std::fs::create_dir_all(ship.parent().unwrap()).unwrap();
        std::fs::copy(data_dir().join(SHIP_FILE), &ship).unwrap();
        let own = dir.join(HOME_DESIGNS_DIR).join("homestead.ron");
        std::fs::create_dir_all(own.parent().unwrap()).unwrap();
        std::fs::write(&own, crate::embedded_data::get_embedded("homes/homestead.ron").expect("the own home is built in")).unwrap();
        dir
    }

    /// THE SHIPPED DEFAULT IS NEVER SAVED OVER (ship homes increment 2 review, finding 5). The
    /// editor's Save writes this player's own home (data/homes/<kind>.ron); the shipped default a
    /// neighbour is drawn as (`HomeDesign::built_in`) is a file of its own
    /// (`embedded_data::SHIPPED_HOMES_DIR`), which a Save leaves byte for byte, and which is the
    /// very file the exe embeds. Before, the two were one file: a Save in a repo checkout became
    /// every neighbour's home at the next build, and every rig refused the exe as stale until
    /// then. Seen red 2026-10-04 with the shipped default at data/homes/homestead.ron (the
    /// c98c5465b layout): "the editor's Save wrote over the shipped default
    /// homes/homestead.ron".
    #[test]
    fn save_assembled_never_writes_the_shipped_home_default() {
        use crate::embedded_data::{shipped_home_design, SHIPPED_HOMES_DIR};
        let dir = temp_data_dir("shipped_home_untouched");
        let shipped_rel = format!("{SHIPPED_HOMES_DIR}/homestead.ron");
        let shipped = dir.join(&shipped_rel);
        std::fs::create_dir_all(shipped.parent().unwrap()).unwrap();
        std::fs::copy(data_dir().join(&shipped_rel), &shipped).unwrap();
        let before = std::fs::read(&shipped).unwrap();
        let walls = HomeDesign::built_in("homestead").unwrap().body.walls.len();
        let mut ship = ShipStructure::load_and_assemble(&dir, None).expect("assembles");
        let hi = ship.home_zone_index();
        ship.zones[hi].body.walls.push(ron::from_str("(a: (1.0, 1.0), b: (3.0, 1.0))").unwrap());
        ship.save_assembled(&dir, false).expect("a Save writes the home");
        assert!(std::fs::read(&shipped).unwrap() == before, "the editor's Save wrote over the shipped default {shipped_rel}");
        let own = HomeDesign::load(&dir, "homestead").expect("the saved home loads");
        assert_eq!(own.body.walls.len(), walls + 1, "the player's own home has the new wall");
        assert_eq!(HomeDesign::built_in("homestead").unwrap().body.walls.len(), walls, "the neighbours' default does not");
        // The file under data/ that a person edits to change the default is the one the exe embeds.
        let repo = std::fs::read_to_string(data_dir().join(&shipped_rel)).unwrap().replace("\r\n", "\n");
        assert_eq!(repo, shipped_home_design("homestead").unwrap().replace("\r\n", "\n"), "data/{shipped_rel} is not what the exe embeds");
    }

    /// THE CABIN (ship homes increment 6): the shipped cabin is England's single bedroom, a floor
    /// of at least 7.5 m2, at least 2.15 m wide and at least 2.3 m from floor to ceiling
    /// (Nationally Described Space Standard, 2015), measured as the standard measures, INSIDE its
    /// walls: the box is drawn to the middle of its shell, which is centred on the box's edge, so
    /// half the shell's thickness comes off each side (3.15 x 2.5 m outside, 3.09 x 2.44 m inside,
    /// 7.54 m2); its door is in the west wall's middle, where every cabin's hall runs; you arrive
    /// inside it; and the player's own copy (data/homes/cabin.ron) and the file the exe embeds
    /// are that design.
    ///
    /// Seen red 2026-10-12 with the shipped cabin's depth 2.0: "the cabin's floor is 6 m2, not
    /// 7.5" failed (Wave 0's test, which measured the box). Lane 1A measures inside the shell;
    /// seen red 2026-10-12 on Wave 0's 3.0 x 2.5 m cabin: "the cabin's floor inside its walls is
    /// 7.1736 m2 (2.94 x 2.44 m), under the standard's 7.5".
    #[test]
    fn a_cabin_is_seven_and_a_half_square_metres() {
        use crate::embedded_data::{shipped_home_design, SHIPPED_HOMES_DIR};
        let cabin = HomeDesign::built_in("cabin").expect("the cabin is built in");
        let b = &cabin.body;
        let shell = b.shell_resolved_thickness();
        assert!((shell - 0.06).abs() < 1e-4, "the cabin's steel shell is 0.06 m thick: {shell}");
        let (inside_w, inside_d) = (b.width - shell, b.depth - shell);
        let floor = inside_w * inside_d;
        assert!(floor >= 7.5, "the cabin's floor inside its walls is {floor} m2 ({inside_w} x {inside_d} m), under the standard's 7.5");
        assert!(inside_w.min(inside_d) >= 2.15, "the cabin is {} m wide inside, under 2.15", inside_w.min(inside_d));
        assert!(b.height - shell >= 2.3, "the cabin is {} m from floor to ceiling, under 2.3", b.height - shell);
        assert_eq!(cabin.door, (0.0, b.depth / 2.0), "the door is in the west wall's middle");
        assert_eq!(cabin.door_lat((80.5, 0.0, 88.0)), Ok(89.25), "a cabin at z 88 meets its hall at lat 89.25");
        let (sx, sz) = b.spawn.expect("the cabin has a spawn");
        assert!(sx > 0.0 && sx < b.width && sz > 0.0 && sz < b.depth, "you arrive inside the cabin, not at ({sx}, {sz})");
        assert!(!b.roof_is_glass(), "an inner cabin has a ceiling, not a skylight");
        assert_eq!(b.zones.len(), 1);
        assert_eq!((b.zones[0].label.as_str(), b.zones[0].room_type.as_deref()), ("Cabin", Some("bedroom")));
        let own = HomeDesign::load(&data_dir(), "cabin").expect("data/homes/cabin.ron loads");
        assert_eq!(ron::ser::to_string(&own).unwrap(), ron::ser::to_string(&cabin).unwrap(), "the player's own cabin starts as the shipped one");
        let repo = std::fs::read_to_string(data_dir().join(format!("{SHIPPED_HOMES_DIR}/cabin.ron"))).unwrap().replace("\r\n", "\n");
        assert_eq!(repo, shipped_home_design("cabin").unwrap().replace("\r\n", "\n"), "data/homes/shipped/cabin.ron is not what the exe embeds");
    }

    /// The plot a home of a kind goes on when nothing names one: the default plot when it is of
    /// that kind, else the first plot of that kind, else none.
    ///
    /// Seen red 2026-10-12 with the default plot skipped (always the first of the kind): "the
    /// default plot, when it is a cabin" failed (p3 for p5).
    #[test]
    fn the_default_plot_of_a_kind_is_the_default_when_it_is_that_kind() {
        let mut ship = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        assert_eq!(ship.default_plot_of_kind("homestead").as_deref(), Some("p1"), "the shipped default plot");
        assert_eq!(ship.default_plot_of_kind("palace"), None, "no plot of that kind");
        for id in ["p3", "p5"] {
            ship.plots.iter_mut().find(|p| p.id == id).expect("the plot").kind = "cabin".into();
        }
        assert_eq!(ship.default_plot_of_kind("cabin").as_deref(), Some("p3"), "the first cabin plot");
        ship.default_plot = Some("p5".into());
        assert_eq!(ship.default_plot_of_kind("cabin").as_deref(), Some("p5"), "the default plot, when it is a cabin");
        assert_eq!(ship.default_plot_of_kind("homestead").as_deref(), Some("p1"), "the first homestead, the default being a cabin");
    }

    /// Saving splits the ship again: the home design always, the ship file only with the Dev
    /// mode's ShipStructureEditing, and the home's origin never (a home moved in the editor is
    /// back on its plot after a reload). A ship that would not load again is not written.
    /// Red check, run: writing the ship file regardless of `ship_scope` fails the second
    /// assertion (the Normal-mode save changed the hangar district).
    #[test]
    fn saving_writes_the_home_always_and_the_ship_only_in_dev_mode() {
        let dir = temp_data_dir("save_split");
        let ship_path = dir.join(SHIP_FILE);
        let ship_bytes = std::fs::read(&ship_path).unwrap();
        let mut ship = ShipStructure::load_and_assemble(&dir, None).expect("assembles");
        let hi = ship.home_zone_index();
        ship.zones[hi].body.walls.push(ron::from_str("(a: (1.0, 1.0), b: (3.0, 1.0))").unwrap());
        ship.zones[hi].origin = (500.0, 0.0, 500.0); // moved in the editor: never saved
        ship.districts[1].origin.0 += 7.0; // the hangar moved (the Normal-mode defect)
        let note = ship.save_assembled(&dir, false).expect("a Normal-mode save writes the home");
        assert_eq!(note, "Saved your home.");
        assert_eq!(std::fs::read(&ship_path).unwrap(), ship_bytes, "Normal mode never writes the ship file");
        let back = ShipStructure::load_and_assemble(&dir, None).expect("reassembles");
        let bh = &back.zones[back.home_zone_index()];
        assert_eq!(bh.body.walls.len(), ship.zones[hi].body.walls.len(), "the new wall was saved");
        assert_eq!(bh.origin, (0.0, 0.0, 0.0), "the home is back on its plot");

        // Dev mode writes the ship too, and it loads again.
        ship.zones[hi].origin = (0.0, 0.0, 0.0);
        ship.save_assembled(&dir, true).expect("a Dev save writes both");
        let back = ShipStructure::load_and_assemble(&dir, None).expect("reassembles");
        assert_eq!(back.districts[1].origin.0, ship.districts[1].origin.0, "the district edit was saved");
        // Every plot, the cabins of increment 6 among them (28 since lane 1A, 12 before it).
        assert!(back.zone_index("street-1").is_some() && back.plots.len() == ship.plots.len() && back.plots.len() >= 28);

        // An overlapping ship is refused, and the file on disk stays as it was. (The street
        // dragged into the Commons: its own corridor no longer resolves and is left out of the
        // copy, and the two zones overlap.)
        let before = std::fs::read(&ship_path).unwrap();
        let si = ship.zone_index("street-1").unwrap();
        ship.zones[si].origin = (65.0, 0.0, 70.0);
        let e = ship.save_assembled(&dir, true).expect_err("an overlapping ship is not saved");
        assert!(e.contains("the ship was NOT saved") && e.contains("overlaps"), "got: {e}");
        assert_eq!(std::fs::read(&ship_path).unwrap(), before);
    }

    /// The Dev Plots and Districts panel edits the ship file through these, and what it does
    /// survives a save and a reload (the critic's review of 1a: plots and districts had no
    /// in-app control). Moving the home's own plot moves the home, its corridor (still first,
    /// still the plot's door, its lat following the door point) and its spawn; saving writes the
    /// moved plot; reloading puts the home there. A plot moved to where it would not take the
    /// design (east of its street, door facing away) is refused by the save, and the file stays
    /// as it was. The home's corridor is the plot's door (`is_plot_door`); a street corridor is
    /// not. Districts add with fresh ids and remove. Red checks, run: dropping the
    /// `sync_home_to_plot` call from `move_plot` fails "the home moved with its plot" (left
    /// (0, 0, 0), right (0, 0, -10)); dropping the every-plot-still-assembles check from
    /// `save_assembled` fails "a ship whose p2 would not load is not saved" (the save said "Saved
    /// your home and the ship.").
    #[test]
    fn dev_plot_and_district_edits_move_the_home_and_save_through_the_ship_file() {
        let dir = temp_data_dir("plot_edits");
        let ship_path = dir.join(SHIP_FILE);
        let mut ship = ShipStructure::load_and_assemble(&dir, None).expect("assembles at p1");
        let spawn_before = ship.home_spawn_world().expect("a spawn");
        assert!(ship.is_plot_door(&ship.corridors[0]), "corridor 0 is the home's door");
        assert!(ship.corridors.iter().skip(1).all(|c| !ship.is_plot_door(c)), "the street's corridor is not");

        assert!(ship.move_plot("p1", (0.0, 0.0, -10.0)));
        let home = &ship.zones[ship.home_zone_index()];
        assert_eq!(home.origin, (0.0, 0.0, -10.0), "the home moved with its plot");
        assert!(ship.is_plot_door(&ship.corridors[0]) && (ship.corridors[0].lat - 30.0).abs() < 1e-4, "its corridor is first, lat 30: {:?}", ship.corridors[0]);
        assert_eq!(ship.corridors.iter().filter(|c| ship.is_plot_door(c)).count(), 1, "one home corridor, not two");
        assert!((ship.home_spawn_world().unwrap() - spawn_before - Vec3::new(0.0, 0.0, -10.0)).length() < 1e-4, "the spawn moved with it");
        assert_eq!(ship.plot_check("p1"), Some(Ok(())));
        assert_eq!(ship.validate(), Ok(()));
        ship.save_assembled(&dir, true).expect("a Dev save writes the moved plot");
        let back = ShipStructure::load_and_assemble(&dir, None).expect("reassembles");
        assert_eq!(back.zones[back.home_zone_index()].origin, (0.0, 0.0, -10.0), "the home loads on its moved plot");

        // Turned away from its street: the panel says why, and the save refuses it. (East of the
        // street at z 300: since ship homes increment 6 the cabin block stands there at z 88 to
        // 136, and a p2 moved onto it would be refused for the overlap instead.)
        let before = std::fs::read(&ship_path).unwrap();
        assert!(ship.move_plot("p2", (80.0, 0.0, 300.0)));
        let why = ship.plot_check("p2").expect("p2 takes the homestead").expect_err("p2 faces away from its street");
        assert!(why.contains("not through the homestead design's door"), "got: {why}");
        let e = ship.save_assembled(&dir, true).expect_err("a ship whose p2 would not load is not saved");
        assert!(e.contains("plot 'p2' would not take the homestead home"), "got: {e}");
        assert_eq!(std::fs::read(&ship_path).unwrap(), before, "the file on disk is unchanged");
        assert!(ship.move_plot("p2", (0.0, 0.0, 99.0)) && ship.plot_check("p2") == Some(Ok(())), "and back again");

        // Plots add beyond their model and remove (never the home's own). The model is the last
        // homestead, p12 (twelve homesteads since 2026-10-04; the sixteen cabins of increment 6
        // follow it in the file, and the new plot takes the first free "p" number).
        let plots_before = ship.plots.len();
        let p13 = ship.add_plot_like("p12", 10.0).expect("a copy of p12");
        assert_eq!(p13, "p13");
        let added = ship.plots.iter().find(|p| p.id == "p13").unwrap();
        assert_eq!((added.origin, added.door.lat), ((0.0, 0.0, 1188.0), 1228.0), "past p12 by its depth and the gap, the lat with it");
        assert!(!ship.remove_plot("p1"), "the home's own plot stays");
        assert!(ship.remove_plot("p13") && ship.plots.len() == plots_before);

        // Districts.
        let id = ship.add_district("hangar", (200.0, 0.0, 0.0), (40.0, 10.0, 40.0));
        assert!(id.starts_with("hangar-") && ship.districts.iter().any(|d| d.id == id));
        assert_ne!(ship.add_district("hangar", (0.0, 0.0, 0.0), (1.0, 1.0, 1.0)), id, "a fresh id each time");
        assert!(ship.remove_district(&id) && !ship.districts.iter().any(|d| d.id == id));
    }

    /// The districts draw at ship level: each non-residential district's filler, and nothing
    /// for a residential one (its home-clone tiling overlapped the Commons; increment 2 draws one
    /// shell per real plot). The clones only ever came from a residential zone inside the home
    /// BODY, which tiles them (`generate_zone_filler`); at ship level nothing does, and
    /// zone_filler.ron has no residential entry, so the skip in `district_fillers` is a second
    /// guard. Red check, run: put a residential zone back into data/homes/homestead.ron's
    /// body (where res-1 sat before the split) and the last assertion fails.
    #[test]
    fn districts_draw_their_fillers_but_residential_ones_draw_nothing() {
        let ship = shipped_ship();
        assert_eq!(ship.districts.len(), 13, "the 13 districts moved up to ship level");
        assert!(!HomeStructure::district_fillers(&ship.districts).is_empty(), "the hangar, the Concourse... draw");
        let residential: Vec<Zone> = ship.districts.iter().filter(|d| d.type_id == "residential").cloned().collect();
        assert_eq!(residential.len(), 1);
        assert!(HomeStructure::district_fillers(&residential).is_empty(), "no home clones over the Commons");
        let home_body = &ship.zones[ship.home_zone_index()].body;
        assert!(
            home_body.zones.iter().all(|z| z.type_id != "residential"),
            "a residential zone in the home body tiles home clones: {:?}",
            home_body.zones.iter().filter(|z| z.type_id == "residential").map(|z| z.id.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_shipped_ron_has_no_wall_coplanar_with_a_corridor_mouth() {
        // Guards the other half of the migration: the two authored door walls that used to sit ON
        // the perimeter at the corridor mouths (home (55,38)-(55,42), commons (0,18)-(0,22)) are
        // gone. If someone re-authors a wall flush with a mouth, the z-fight comes back. Every
        // corridor is checked (the street's runs along Z, so its mouth planes are z planes).
        let ship = shipped_ship();
        for c in &ship.corridors {
            let g = ship.corridor_geometry(c).expect("resolves");
            // (along the run, across the run) for a point (x, z).
            let split = |x: f32, z: f32| match g.axis {
                CorridorAxis::X => (x, z),
                CorridorAxis::Z => (z, x),
            };
            for zi in [g.from_zone_idx, g.to_zone_idx] {
                let z = &ship.zones[zi];
                let o = z.origin_vec();
                for wall in &z.body.walls {
                    let (a_run, a_lat) = split(o.x + wall.a.0, o.z + wall.a.1);
                    let (b_run, b_lat) = split(o.x + wall.b.0, o.z + wall.b.1);
                    for plane in [g.start, g.end] {
                        let on_plane = (a_run - plane).abs() < 1e-3 && (b_run - plane).abs() < 1e-3;
                        let overlaps_mouth = a_lat.min(b_lat) < g.lat + g.door_from.0 * 0.5
                            && a_lat.max(b_lat) > g.lat - g.door_from.0 * 0.5;
                        assert!(
                            !(on_plane && overlaps_mouth),
                            "zone '{}' has an authored wall coplanar with the {} -> {} mouth at {plane}",
                            z.id,
                            c.from_zone,
                            c.to_zone
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod lighting_watts_tests {
    use super::*;

    /// v0.967 (homestead increment 5): the shipped structure's switched-on
    /// lights must sum to a sane, NONZERO wattage through the shipped
    /// light-type table - the house lighting is real load on the power
    /// meter, and a light-type rename that silently zeroes the bill fails
    /// here instead of in the HUD.
    #[test]
    fn shipped_lights_draw_real_watts() {
        // The ship the game runs (the ship file + the homestead on its default plot).
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let s = ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
        let watts = s.lighting_watts(|id| {
            crate::renderer::light::light_type(id).map(|t| t.watts).unwrap_or(0.0)
        });
        assert!(
            (100.0..2000.0).contains(&watts),
            "shipped lighting should draw a realistic LED-household load, got {watts} W"
        );
        // Every shipped light type must carry a wattage (a new entry with
        // watts omitted defaults to 0 = free power, which is a lie).
        for t in crate::renderer::light::light_types() {
            assert!(t.watts > 0.0, "light type {} has no wattage", t.id);
        }
    }
}

/// Increment 1b of docs/design/ship-homes-and-logistics.md: what a relay hands out (plots, their
/// arrival points, the Commons for guests) and the ship fingerprint both sides compare.
#[cfg(test)]
mod plot_handout_tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The shipped home design names its door, and the relay's arrival for that door on every
    /// plot is exactly where the game's camera lands once the home is assembled there (the game's
    /// own path, `assemble` then `home_spawn_world`), inside that plot's box.
    ///
    /// Since ship homes increment 6 (lane 1A) the ship holds cabins too: each plot is checked
    /// with the door of its own kind's shipped design, which is what that plot's holder's game
    /// names.
    #[test]
    fn every_plot_spawn_is_where_the_game_stands_on_it() {
        let plots = ShipPlots::load(&data_dir()).expect("the shipped plots load");
        assert!(plots.plots.len() >= 2, "two plots at least: {:?}", plots.plots);
        let door = ShipStructure::load_and_assemble_shipped(&data_dir(), None).unwrap().home_arrival_local();
        assert!(door.is_some(), "the shipped home design names its door");
        for p in &plots.plots {
            let ship = ShipStructure::load_and_assemble_shipped(&data_dir(), Some(&p.id)).expect("assembles");
            let cam = ship.home_spawn_world().expect("the design has a spawn");
            let door = HomeDesign::built_in(&p.kind).and_then(|d| d.body.spawn);
            assert!(door.is_some(), "the shipped {} design names its door", p.kind);
            let relay = p.arrival(door);
            assert!((cam - relay).length() < 1e-4, "{}: relay {relay:?}, game {cam:?}", p.id);
            // And it is inside the plot's own box.
            let (lo, hi) = ship.plots.iter().find(|q| q.id == p.id).unwrap().aabb();
            assert!(cam.cmpge(lo).all() && cam.cmple(hi).all(), "{}'s spawn {cam:?} is outside its box", p.id);
        }
        // The design doc's worked numbers (section 2.4): p2's spawn is (53.5, 1.7, 139.5).
        let p2 = plots.plot("p2").expect("p2").arrival(door);
        assert!((p2 - Vec3::new(53.5, 1.7, 139.5)).length() < 1e-4, "p2 spawn {p2:?}");
        // A guest arrives in the Commons: the middle of its 34 x 55 m box at (65, 0, 20).
        let g = plots.guest_spawn.expect("the ship has a Commons");
        // The Commons' own spawn (increment 4: it used to be its middle, inside the room block).
        assert!((g - Vec3::new(87.5, 1.7, 67.5)).length() < 1e-4, "guest spawn {g:?}");
        assert_eq!(plots.ship_id, "mothership-1");
    }

    /// The player's own door, as the game sends it (`home_arrival_local`), arrives on any plot
    /// exactly where the game's camera lands when its home is assembled there, and a door with
    /// a moved spawn (the build-mode avatar) arrives at the moved point, not the default
    /// design's. A door off the plot is kept on it; one that is not a number is ignored. Seen
    /// red 2026-10-03 with `arrival` ignoring the door (the first 1b relay, which always used
    /// the default design's spawn): "p1: relay Vec3(53.5, 1.7, 40.5), game Vec3(12.5, 1.7, 30.0)".
    /// No door named now arrives in the middle of the plot (the second review); with `arrival`
    /// put back to the default design's door for that: "no door named: the middle of the plot".
    /// (The homestead goes on the homestead plots only: since increment 6 the ship holds cabins,
    /// which `assemble` refuses it.)
    #[test]
    fn the_players_own_door_arrives_where_their_home_is_entered() {
        let plots = ShipPlots::load(&data_dir()).expect("the shipped plots load");
        let mut ship = ShipStructure::load_and_assemble_shipped(&data_dir(), None).expect("assembles on p1");
        let home = ship.home_zone_index();
        ship.zones[home].body.spawn = Some((12.5, 30.0)); // the player moved their door
        let door = ship.home_arrival_local().expect("an assembled home has a door");
        assert_eq!(door, (12.5, 30.0));
        for p in plots.plots.iter().filter(|p| p.kind == "homestead") {
            let there = ship.ship_file().assemble(ship.home_design().unwrap(), &p.id).expect("fits");
            let cam = there.home_spawn_world().unwrap();
            assert!((p.arrival(Some(door)) - cam).length() < 1e-4, "{}: relay {:?}, game {cam:?}", p.id, p.arrival(Some(door)));
            assert_eq!(there.home_arrival_local(), Some(door), "plot-local, so the same on every plot");
        }
        let p1 = plots.plot("p1").unwrap();
        let middle = Vec3::new(p1.origin.0 + p1.size.0 * 0.5, 1.7, p1.origin.2 + p1.size.2 * 0.5);
        assert!((p1.arrival(None) - middle).length() < 1e-4, "no door named: the middle of the plot");
        assert!((p1.arrival(Some((f32::NAN, 3.0))) - middle).length() < 1e-4, "not a number: ignored");
        let edge = p1.arrival(Some((1000.0, -5.0)));
        assert!((edge - Vec3::new(p1.origin.0 + p1.size.0, 1.7, p1.origin.2)).length() < 1e-4, "kept on the plot: {edge:?}");
    }

    /// A home with NO authored door names none in its join, and the relay and the game then both
    /// take the middle of the plot actually handed out, on plots of any size. Here the last plot
    /// (p12, at First Street's far end, with room to grow; it was p2 until the twelve plots of
    /// 2026-10-04 put p3 10 m past it) is made larger than the first (planned apartment plots will
    /// differ in size), the home is built on the first, and the relay hands out the larger one.
    ///
    /// Seen red 2026-10-03 with `home_arrival_local` put back to sending the middle of the plot
    /// the home was built on (the 65b3e2c0c game): "a doorless home on the larger p2: the relay
    /// spawns at Vec3(27.5, 1.7, 143.5), the game stands at Vec3(31.5, 1.7, 159.0)".
    #[test]
    fn a_home_with_no_door_arrives_in_the_middle_of_the_plot_it_is_given() {
        let mut file = ShipStructure::load_ship_file(&data_dir()).unwrap();
        // The last HOMESTEAD plot (since increment 6 the cabins follow the homesteads in the file).
        let big = file.plots.iter().rposition(|p| p.kind == "homestead").expect("a homestead plot");
        let id = file.plots[big].id.clone();
        // Wider up to the street (x 65) and deeper: 63 x 120 m against p1's 55 x 89.
        file.plots[big].size.0 += 8.0;
        file.plots[big].size.2 += 31.0;
        let mut design = ShipStructure::load_and_assemble_shipped(&data_dir(), None).unwrap().home_design().unwrap();
        design.body.spawn = None;
        let ship = file.clone().assemble(design.clone(), "p1").expect("the doorless home stands on p1");
        // The relay's side: what it spawns at on the plot it hands out, from what the join says.
        let relay = ShipPlots::of_ship(&file).plot(&id).unwrap().arrival(ship.home_arrival_local());
        // The game's side: where its home's arrival point stands on that plot.
        let game = ShipStructure::plot_spawn(&file.plots[big], &design);
        assert!(
            (relay - game).length() < 1e-4,
            "a doorless home on the larger {id}: the relay spawns at {relay:?}, the game stands at {game:?}"
        );
        assert_eq!(ship.home_arrival_local(), None, "a home with no door names none");
    }

    /// A relay in a folder with no data (a bare exe run with --headless in an empty folder; the
    /// rigs' throwaway relay has had the tree's data/ mirrored into its folder since 2026-10-03,
    /// scripts/lib/throwaway-relay.js `mirrorData`) hands out the same plots and names the same
    /// ship, from the copies built into the exe.
    #[test]
    fn a_relay_with_no_data_folder_hands_out_the_same_plots() {
        let empty = crate::test_temp::dir("no_data");
        let built_in = ShipPlots::load(&empty).expect("the built-in copies load");
        let on_disk = ShipPlots::load(&data_dir()).expect("the data folder loads");
        assert_eq!(built_in, on_disk, "the exe's copy is the data folder's ship");
    }

    /// MORE HOMES ALONG FIRST STREET (2026-10-04, the operator's accepted recommendation: "add
    /// plots along First Street (about ten) so more than two people can have homes"). The shipped
    /// ship has twelve homestead plots: p1 and p2 exactly where they always were (a relay's stored
    /// claims name plots by id, `game_plots`), and p3 to p12 down the west side of First Street,
    /// each 55 x 89 m, 99 m apart as p1 and p2 are, with its door on the street. Only the west
    /// side: the homestead's door is in its EAST wall and a plot cannot turn yet (section 2.2 of
    /// the design, "a quarter-turn yaw later"), so a home east of the street would face away
    /// from it, which `assemble` refuses (the critic's review of 1a). The file validates (no
    /// plot, zone or corridor overlaps another); every plot takes the shipped homestead with its
    /// corridor leaving through the design's own door; First Street reaches past the last door;
    /// and the copy built into the exe, which a relay with no data folder hands out, is the same
    /// ship.
    ///
    /// Seen red 2026-10-04 on the two-plot ship file: "the shipped ship has twelve plots, in the
    /// order a relay hands them out: [\"p1\", \"p2\"]".
    ///
    /// Since ship homes increment 6 (lane 1A) the ship file holds sixteen cabin plots as well,
    /// after the homesteads (`the_shipped_ship_has_a_cabin_block_beside_first_street`), so this
    /// test reads the HOMESTEAD plots: they are still the first twelve in the file, p1 to p12.
    #[test]
    fn the_shipped_ship_has_twelve_homes_along_first_street() {
        let mut file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        file.validate().expect("the ship file validates: no plot, zone or corridor overlaps another");
        let all = file.clone();
        file.plots.retain(|p| p.kind == "homestead");
        let ids: Vec<&str> = file.plots.iter().map(|p| p.id.as_str()).collect();
        let want: Vec<String> = (1..=12).map(|n| format!("p{n}")).collect();
        assert_eq!(ids, want, "the shipped ship has twelve plots, in the order a relay hands them out: {ids:?}");
        assert_eq!(all.plots[..12].iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), want, "the homesteads come first in the file");
        // p1 and p2 never move: a relay's stored claims name them, and the plot a save stands on
        // (`WorldSave::ship_place`) is an id too.
        let plot = |id: &str| file.plots.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((plot("p1").origin, plot("p1").door.zone.as_str(), plot("p1").door.lat), ((0.0, 0.0, 0.0), "commons", 40.0));
        assert_eq!((plot("p2").origin, plot("p2").door.zone.as_str(), plot("p2").door.lat), ((0.0, 0.0, 99.0), "street-1", 139.0));
        assert_eq!(file.default_plot.as_deref(), Some("p1"), "offline play still builds on p1");
        let street = &file.zones[file.zone_index("street-1").expect("First Street is a zone")];
        assert_eq!(street.label, "First Street");
        let design = HomeDesign::built_in("homestead").expect("the shipped homestead is built in");
        for p in &file.plots {
            // 6 m tall since ship homes increment 7: two 3.0 m storeys.
            assert_eq!((p.kind.as_str(), p.size), ("homestead", (55.0, 6.0, 89.0)), "{} is a homestead plot", p.id);
            if p.id != "p1" {
                assert_eq!(p.door.zone, "street-1", "{}'s door opens onto First Street", p.id);
                // West of the street, its east wall facing it across a clear gap.
                assert!(p.origin.0 + p.size.0 < street.origin.0, "{} stands west of First Street", p.id);
                // The street runs past the door, mouth and all.
                let (z0, z1) = (street.origin.2, street.origin.2 + street.body.depth);
                assert!(p.door.lat - p.door.door_width * 0.5 > z0 && p.door.lat + p.door.door_width * 0.5 < z1, "{}'s door is on the street", p.id);
            }
            // Its home stands on it, the door corridor leaving through the design's own door
            // (`assemble` refuses anything else), and its holder arrives at that door, on it.
            let ship = all.clone().assemble(design.clone(), &p.id).unwrap_or_else(|e| panic!("{} takes the shipped homestead: {e}", p.id));
            let door = ship.home_spawn_world().expect("the shipped homestead names its door");
            let (lo, hi) = p.aabb();
            assert!(door.cmpge(lo).all() && door.cmple(hi).all(), "{}'s door {door:?} is on it", p.id);
            assert_eq!(ship.ship_hash(), all.ship_hash(), "the ship is the same ship whichever plot the home stands on");
        }
        // Consecutive plots 10 m apart along z, as p2 sits past p1: room for each corridor, and no
        // two homes' walls flush against each other.
        for w in file.plots.windows(2) {
            assert!((w[1].origin.2 - (w[0].origin.2 + w[0].size.2) - 10.0).abs() < 1e-3, "{} is 10 m past {}", w[1].id, w[0].id);
        }
        // A relay with no data folder hands out exactly these.
        let built_in = ShipStructure::built_in_ship_file("test: the twelve plots").expect("the built-in ship loads");
        assert_eq!(built_in.ship_hash(), all.ship_hash(), "the ship built into the exe is the data folder's");
    }

    /// THE CABIN BLOCK (ship homes increment 6, plan section 3.3): east of First Street's north
    /// end, the hall `cabins-1` ("Cabin Block A") is reached from First Street by its own
    /// corridor, and sixteen cabin plots, c1 to c16 in the file's order after the homesteads,
    /// stand along its east side, each taking the SHIPPED cabin (data/homes/shipped/cabin.ron)
    /// through its own door into the hall: the door corridor leaves the cabin through the
    /// design's door, in its west wall, and arrives on the hall's east wall. Neighbouring cabins
    /// touch, 2.5 m apart, and none overlaps another or the hall; the file validates; whoever
    /// holds a cabin arrives inside it; the hall's washroom is a bathroom; the first cabin is
    /// where a cabin goes when nothing names a plot; and the copy built into the exe, which a
    /// relay with no data folder hands out, is the same ship.
    ///
    /// Seen red 2026-10-12 with c7's door lat moved 1 m north (103.25 for 104.25), which takes its
    /// door off the plot: "the ship file validates: no plot, zone or corridor overlaps another:
    /// \"plot 'c7' door: lat 103.25 m puts the door mouth outside the zones' shared z span; valid:
    /// 103.45 to 105.05 m\""; and moved half a metre (103.75), still on the plot: "c7 takes the
    /// shipped cabin: plot 'c7' door lat 103.75 does not meet the cabin design's door at local
    /// (0.0, 1.25), which needs lat 104.25".
    #[test]
    fn the_shipped_ship_has_a_cabin_block_beside_first_street() {
        let file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        file.validate().expect("the ship file validates: no plot, zone or corridor overlaps another");
        let cabin = HomeDesign::built_in("cabin").expect("the shipped cabin is built in");
        let hall = &file.zones[file.zone_index("cabins-1").expect("the cabin block's hall is a zone")];
        assert_eq!((hall.label.as_str(), hall.purpose.as_str()), ("Cabin Block A", "residence"));
        let (hx0, hx1, hz0, hz1) = (hall.origin.0, hall.origin.0 + hall.body.width, hall.origin.2, hall.origin.2 + hall.body.depth);
        // The hall is reached from First Street by its own corridor.
        let to_hall = file
            .corridors
            .iter()
            .find(|c| (c.from_zone == "street-1" && c.to_zone == "cabins-1") || (c.from_zone == "cabins-1" && c.to_zone == "street-1"))
            .expect("a corridor from First Street to the cabin block");
        let g = file.corridor_geometry(to_hall).expect("the hall's corridor resolves");
        assert_eq!(g.axis, CorridorAxis::X, "it runs across from the street to the hall");
        assert!((g.end.min(g.start) - 75.0).abs() < 1e-3 && (g.end.max(g.start) - hx0).abs() < 1e-3, "from the street's east wall to the hall's west wall: {g:?}");
        // The washroom, behind the hall's end wall.
        let wash = hall.body.zones.iter().find(|v| v.type_id == "room_bathroom").expect("the block has a washroom");
        assert_eq!((wash.label.as_str(), wash.room_type.as_deref()), ("Washroom", Some("bathroom")));
        // Sixteen cabins after the homesteads.
        let ids: Vec<&str> = file.plots.iter().filter(|p| p.kind == "cabin").map(|p| p.id.as_str()).collect();
        let want: Vec<String> = (1..=16).map(|n| format!("c{n}")).collect();
        assert_eq!(ids, want, "sixteen cabin plots in the file's order");
        let first_cabin = file.plots.iter().position(|p| p.kind == "cabin").unwrap();
        assert!(file.plots[..first_cabin].iter().all(|p| p.kind == "homestead"), "the homesteads come first: a relay's older claims name them");
        assert_eq!(file.default_plot_of_kind("cabin").as_deref(), Some("c1"), "a cabin with no place yet goes on the first");
        let cabins: Vec<&Plot> = file.plots.iter().filter(|p| p.kind == "cabin").collect();
        for (i, p) in cabins.iter().enumerate() {
            assert_eq!(p.door.zone, "cabins-1", "{}'s door opens on the hall", p.id);
            assert!(p.origin.0 >= hx1 + CORRIDOR_MIN_RUN - 1e-3, "{} stands east of the hall, across a door corridor", p.id);
            assert!(p.origin.2 >= hz0 && p.origin.2 + p.size.2 <= hz1, "{} stands along the hall", p.id);
            let ship = file.clone().assemble(cabin.clone(), &p.id).unwrap_or_else(|e| panic!("{} takes the shipped cabin: {e}", p.id));
            // `assemble` refuses a corridor that leaves through another wall; it ends on the hall.
            let door = ship.corridor_geometry(&ship.corridors[0]).expect("the cabin's door corridor resolves");
            assert!((door.end_to.x - hx1).abs() < 1e-3 && door.end_to.z > hz0 && door.end_to.z < hz1, "{}'s corridor arrives on the hall's east wall: {:?}", p.id, door.end_to);
            let spawn = ship.home_spawn_world().expect("the shipped cabin names where you arrive");
            let (lo, hi) = p.aabb();
            assert!(spawn.cmpge(lo).all() && spawn.cmple(hi).all(), "whoever holds {} arrives in it, not at {spawn:?}", p.id);
            assert_eq!(ship.ship_hash(), file.ship_hash(), "the ship is the same ship whichever cabin the home stands on");
            if i > 0 {
                let prev = cabins[i - 1];
                assert!((p.origin.2 - (prev.origin.2 + prev.size.2)).abs() < 1e-3, "{} touches {}", p.id, prev.id);
                assert!((p.door.lat - prev.door.lat - 2.5).abs() < 1e-3, "{}'s door is 2.5 m past {}'s", p.id, prev.id);
            }
        }
        // Nothing stands east of the cabins on the ship's floor: the wider cabins crowd nothing.
        let east = cabins.iter().map(|p| p.origin.0 + p.size.0).fold(0.0_f32, f32::max);
        for z in file.zones.iter().filter(|z| z.id != "cabins-1") {
            let (zx0, zz0, zz1) = (z.origin.0, z.origin.2, z.origin.2 + z.body.depth);
            assert!(zx0 + z.body.width <= hx0 || zz1 <= hz0 || zz0 >= hz1 || zx0 >= east + 10.0, "zone {} crowds the cabin block", z.id);
        }
        let built_in = ShipStructure::built_in_ship_file("test: the cabin block").expect("the built-in ship loads");
        assert_eq!(built_in.ship_hash(), file.ship_hash(), "the ship built into the exe is the data folder's");
    }

    /// EVERY HOME REACHES A MESS HALL WITHIN FIVE MINUTES (docs/design/ship-homes-and-logistics.md
    /// section 2.9, the rule that a household without a kitchen of its own still eats; plan 3.3):
    /// from every plot's door, along the ship's corridors, to the middle of the nearest
    /// `mess_hall` volume, at 1.4 m/s, an adult's walking pace. The walk is the corridor graph:
    /// each plot's door corridor (with the plot's own shipped design on it), the ship's corridors
    /// between zones, and a straight line between any two points of one zone (interior walls are
    /// not walked around: the Commons' room block adds about 10 m to a walk to its mess hall).
    ///
    /// Seen red 2026-10-12 on the ship without the South Commons (its zone and corridor taken out
    /// of the loaded file): "over five minutes from the nearest mess hall: p5 5.0 min (423 m), p6
    /// 6.2 min (522 m), p7 7.4 min (621 m), p8 8.6 min (720 m), p9 9.7 min (819 m), p10 10.9 min
    /// (918 m), p11 12.1 min (1017 m), p12 13.3 min (1116 m)". With it the farthest is p5, 4.6
    /// minutes (386 m) from the South Commons.
    #[test]
    fn every_home_reaches_a_mess_hall_within_five_minutes() {
        let file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        let walks = minutes_to_a_mess_hall(&file);
        assert_eq!(walks.len(), file.plots.len(), "every plot's door is on the corridor graph: {walks:?}");
        let over: Vec<String> = walks.iter().filter(|w| w.1 > 5.0).map(|(id, minutes, metres)| format!("{id} {minutes:.1} min ({metres:.0} m)")).collect();
        assert!(over.is_empty(), "over five minutes from the nearest mess hall: {}", over.join(", "));
        // Both mess halls serve someone: the rule needs the South Commons.
        let far = walks.iter().map(|w| w.1).fold(0.0_f32, f32::max);
        assert!(far > 3.0, "the far end of First Street is still a walk: {far:.1} minutes at most");
    }

    /// Each plot's walk from its door to the middle of the nearest mess hall: (plot id, minutes
    /// at 1.4 m/s, metres), along the corridor graph `every_home_reaches_a_mess_hall_within_five_
    /// minutes` describes. A plot whose door does not reach a mess hall is left out.
    fn minutes_to_a_mess_hall(ship: &ShipStructure) -> Vec<(String, f32, f32)> {
        const WALK_M_PER_S: f32 = 1.4;
        let xz = |v: Vec3| glam::Vec2::new(v.x, v.z);
        // Points on the floor and the zone each stands in (None: a plot's door, inside a home).
        let mut nodes: Vec<(glam::Vec2, Option<usize>)> = Vec::new();
        let mut edges: Vec<(usize, usize, f32)> = Vec::new();
        let add = |nodes: &mut Vec<(glam::Vec2, Option<usize>)>, p: glam::Vec2, zone: Option<usize>| {
            nodes.push((p, zone));
            nodes.len() - 1
        };
        for c in &ship.corridors {
            let Ok(g) = ship.corridor_geometry(c) else { continue };
            let a = add(&mut nodes, xz(g.end_from), Some(g.from_zone_idx));
            let b = add(&mut nodes, xz(g.end_to), Some(g.to_zone_idx));
            edges.push((a, b, g.end_from.distance(g.end_to)));
        }
        let mut doors: Vec<(String, usize)> = Vec::new();
        for p in &ship.plots {
            let Ok(g) = ship.plot_door_tube(p, HomeDesign::built_in_ref(&p.kind)) else { continue };
            let a = add(&mut nodes, xz(g.end_from), None);
            let b = add(&mut nodes, xz(g.end_to), Some(g.to_zone_idx));
            edges.push((a, b, g.end_from.distance(g.end_to)));
            doors.push((p.id.clone(), a));
        }
        let mut halls: Vec<usize> = Vec::new();
        for (zi, z) in ship.zones.iter().enumerate() {
            for v in z.body.zones.iter().filter(|v| v.type_id == "mess_hall") {
                let centre = glam::Vec2::new(z.origin.0 + v.origin.0 + v.size.0 * 0.5, z.origin.2 + v.origin.2 + v.size.2 * 0.5);
                halls.push(add(&mut nodes, centre, Some(zi)));
            }
        }
        for i in 0..nodes.len() {
            for j in i + 1..nodes.len() {
                if nodes[i].1.is_some() && nodes[i].1 == nodes[j].1 {
                    edges.push((i, j, nodes[i].0.distance(nodes[j].0)));
                }
            }
        }
        let mut out = Vec::new();
        for (id, start) in doors {
            // Dijkstra on a graph of a few dozen points.
            let mut dist = vec![f32::INFINITY; nodes.len()];
            let mut done = vec![false; nodes.len()];
            dist[start] = 0.0;
            while let Some(u) = (0..nodes.len()).filter(|&i| !done[i] && dist[i].is_finite()).min_by(|&a, &b| dist[a].total_cmp(&dist[b])) {
                done[u] = true;
                for &(a, b, w) in &edges {
                    let v = if a == u { b } else if b == u { a } else { continue };
                    if dist[u] + w < dist[v] {
                        dist[v] = dist[u] + w;
                    }
                }
            }
            let metres = halls.iter().map(|&h| dist[h]).fold(f32::INFINITY, f32::min);
            if metres.is_finite() {
                out.push((id, metres / WALK_M_PER_S / 60.0, metres));
            }
        }
        out
    }

    /// A relay whose ship file on disk does not load (a bad hand edit on the server) keeps the
    /// ship built into it, the one the same version of the game draws, instead of no ship at
    /// all: the third review of 1b found every game's join then refused as "a different ship
    /// from yours". The broken file is moved aside, not overwritten.
    ///
    /// Seen red 2026-10-03 on the a504c5cd9 `ShipPlots::load` (no fallback): "a relay with a
    /// broken ship file keeps the built-in ship: Err(\"...ship_structure.ron did not load; it
    /// was moved aside as ship_structure.invalid-<time>.ron (see logs/run.log)\")".
    #[test]
    fn a_relay_whose_ship_file_does_not_load_keeps_the_built_in_ship() {
        let dir = crate::test_temp::path("bad_ship");
        let file = dir.join(SHIP_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "( this is not a ship").unwrap();
        let got = ShipPlots::load(&dir);
        let built_in = ShipPlots::of_ship(&ShipStructure::built_in_ship_file("test: the built-in ship").unwrap());
        assert!(got.as_ref().is_ok_and(|p| *p == built_in), "a relay with a broken ship file keeps the built-in ship: {got:?}");
        assert!(!file.exists(), "the broken file was moved aside");
        let kept = std::fs::read_dir(file.parent().unwrap()).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().contains("invalid-"));
        assert!(kept, "and kept for recovery by hand");
    }

    /// The fingerprint: the same for the ship file and any ship assembled from it (whichever
    /// plot the home stands on), unchanged by comments, number formatting or the rebuild's corner
    /// normalisation, and changed by a moved plot.
    #[test]
    fn the_ship_hash_names_the_ship_file_and_nothing_else() {
        let file = ShipStructure::load_ship_file(&data_dir()).unwrap();
        let h = file.ship_hash();
        assert_eq!(h.len(), 16);
        for plot in ["p1", "p2"] {
            let ship = ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).unwrap();
            assert_eq!(ship.ship_hash(), h, "assembled on {plot}, the ship is the same ship");
        }
        // Reformatting the text (comments gone, pretty printing) is the same ship.
        let reprinted = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        let reparsed: ShipStructure = ron::from_str(&reprinted).unwrap();
        assert_eq!(reparsed.ship_hash(), h);
        // The game's rebuild normalises every corner (engine/home_meshes.rs rebuild_homestead),
        // and a rejoin's welcome is checked against the ship after that: the shipped corners are
        // already on the grid, so the hash survives it.
        let mut rebuilt = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        for z in rebuilt.zones.iter_mut() {
            for w in z.body.walls.iter_mut() {
                w.a = crate::ship::home_structure::quantize_corner(w.a);
                w.b = crate::ship::home_structure::quantize_corner(w.b);
            }
        }
        assert_eq!(rebuilt.ship_hash(), h, "the rebuild's corner normalisation changed the ship");
        // A moved plot is a different ship: positions would no longer agree.
        let mut moved = file.clone();
        moved.plots[1].origin.2 += 1.0;
        assert_ne!(moved.ship_hash(), h);
    }

    /// THE OPEN PLOTS ARE NEVER PART OF THE SHIP (ship homes 5b, plan section 3.7). Which
    /// neighbours' plots this game may walk into, and which one it lights, is its own business:
    /// with p1 open and lit on a ship assembled on p2, the hash a relay compares and the ship file
    /// an editor writes are exactly those of the same ship with nothing open, and the assembled
    /// ship's own text never mentions them either.
    /// Seen red 2026-10-12 with `#[serde(skip)]` on `open` replaced by `#[serde(default)]`:
    /// "opening p1 changed the ship's hash", left "d7af1b57e32972e2", right "a0ef8a6b2a12dd81" (the
    /// hashes of that run).
    #[test]
    fn the_open_plots_are_never_part_of_the_ship() {
        let closed = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let mut open = closed.clone();
        open.open = OpenPlots { plots: ["p1".to_string()].into_iter().collect(), lit: Some("p1".into()) };
        assert_ne!(open.open, closed.open, "p1 really is open on one of them");
        assert_eq!(open.ship_hash(), closed.ship_hash(), "opening p1 changed the ship's hash");
        let text = |s: &ShipStructure| ron::ser::to_string_pretty(s, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(text(&open.ship_file()), text(&closed.ship_file()), "the ship file is the same");
        assert_eq!(text(&open), text(&closed), "the assembled ship's own text is the same");
    }
}

/// Ship homes increment 7, Wave 0 (docs/design/ship-homes-increment-7-plan.md section 2): the
/// storey fields leave every shipped file, and so the ship's hash, exactly as they were.
#[cfg(test)]
mod storey_fields_tests {
    use super::*;

    /// THE STOREY FIELDS MOVE NO SHIPPED FILE. `HomeStructure::floors` and `InteriorWall::base`
    /// are left out of a file while unused, so the ship file and every shipped home design
    /// re-serialize byte for byte as they did, and the ship hashes as it did: a game built before
    /// storeys still meets this ship as the same ship.
    ///
    /// The values are PINNED, computed on the code before Wave 0 (commit 510d91c8f, 2026-10-20) by
    /// a throwaway capture over the same calls, to prove the new fields do not move them. They are
    /// FNV-1a 64 of the RON text: compact (`ron::ser::to_string`, what `ship_hash` hashes) and
    /// pretty with struct names off (what an editor Save writes). When a shipped file changes ON
    /// PURPOSE (lane R grows the homestead plots in the ship file, lane C1 gives the homestead its
    /// upper floor), replace that file's pins with the values this test prints and say why in the
    /// commit; a pin that moves when nobody meant to change the file is the bug it is here for.
    ///
    /// Seen red 2026-10-20 with `floors` written even when empty: all seven moved, among them
    /// "ship_hash, data/blueprints/ship_structure.ron: pinned effbdfbec3360115, now a327f9b37c384935"
    /// and "homes/shipped/homestead.ron, compact: pinned 7f2c00ab2c30e0ce, now 2272e8546b075515".
    ///
    /// MOVED ON PURPOSE 2026-10-20 (lane R of increment 7): the twelve homestead plots grew from
    /// 3.0 to 6.0 m in the ship file, so its three pins changed: "ship_hash, ...ship_structure.ron:
    /// pinned effbdfbec3360115, now 507bc93bce4f4dbf", the same for the ship built into the exe, and
    /// "ship file, pretty: pinned 2f24b7a1c04beedd, now f25a9d8398e84427". The home designs' four
    /// pins did not move. effbdfbec3360115 is now an older game's ship
    /// (`an_older_game_is_refused_as_another_ship`).
    #[test]
    fn the_storey_fields_move_no_shipped_file() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let pretty = || ron::ser::PrettyConfig::default().struct_names(false);
        let fnv = |text: String| format!("{:016x}", fnv1a64(text.as_bytes()));
        let ship = ShipStructure::load_ship_file(&data).expect("the ship file loads");
        let built_in: ShipStructure = ron::from_str(crate::embedded_data::SHIP_STRUCTURE_RON).expect("the built-in ship parses");
        let mut now: Vec<(String, String)> = vec![
            ("ship_hash, data/blueprints/ship_structure.ron".into(), ship.ship_hash()),
            ("ship_hash, the ship built into the exe".into(), built_in.ship_hash()),
            ("ship file, pretty".into(), fnv(ron::ser::to_string_pretty(&ship, pretty()).unwrap())),
        ];
        for kind in ["homestead", "cabin"] {
            let design = HomeDesign::built_in(kind).expect("the shipped design is built in");
            now.push((format!("homes/shipped/{kind}.ron, compact"), fnv(ron::ser::to_string(&design).unwrap())));
            now.push((format!("homes/shipped/{kind}.ron, pretty"), fnv(ron::ser::to_string_pretty(&design, pretty()).unwrap())));
        }
        const PINNED: [&str; 7] = [
            "507bc93bce4f4dbf",
            "507bc93bce4f4dbf",
            "f25a9d8398e84427",
            // The homestead's upper floor (increment 7, lane C1, 2026-10-20) moved these two on
            // purpose; before it they were 7f2c00ab2c30e0ce and d015569462471165.
            "069cc7d553362752",
            "6d0d37b140c4343b",
            "da22405ff5cb9a36",
            "ffb704a6e5cf699b",
        ];
        let moved: Vec<String> = now
            .iter()
            .zip(PINNED)
            .filter(|((_, got), pin)| got != pin)
            .map(|((what, got), pin)| format!("  {what}: pinned {pin}, now {got}"))
            .collect();
        assert!(moved.is_empty(), "a shipped file no longer serializes as it did:\n{}", moved.join("\n"));
    }

    /// A TWO-STOREY HOMESTEAD FITS ITS PLOT (ship homes increment 7, plan decision 3): every
    /// homestead plot of the ship file is 6.0 m tall, so the homestead body at 6.0 m (two 3.0 m
    /// storeys, lane C1's upper floor) fits each of them and a 6.5 m one fits none; the cabin
    /// plots stay one 3.0 m storey, and a 6 m body fits none of them. An older game, whose ship
    /// file still has 3 m plots, hashes differently, so a relay running this file meets it as a
    /// different ship (`ship_hash`, checked in the game's welcome), which is the refusal the
    /// increment accepts while nobody is using the ship.
    ///
    /// Seen red 2026-10-20 on the ship file before the plots grew: "a 6 m homestead does not fit
    /// p1: Err(\"the homestead design (55 x 89 x 6 m) does not fit plot 'p1' ((55.0, 3.0, 89.0))\")".
    #[test]
    fn a_two_storey_homestead_fits_its_plot() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let ship = ShipStructure::load_ship_file(&data).expect("the ship file loads");
        let mut design = HomeDesign::built_in("homestead").expect("the shipped homestead");
        let homesteads: Vec<&Plot> = ship.plots.iter().filter(|p| p.kind == "homestead").collect();
        assert_eq!(homesteads.len(), 12, "twelve homestead plots");
        for plot in &homesteads {
            design.body.height = 6.0;
            let fits = design_fits_plot(&design, plot);
            assert!(fits.is_ok(), "a 6 m homestead does not fit {}: {fits:?}", plot.id);
            design.body.height = 6.5;
            assert!(design_fits_plot(&design, plot).is_err(), "a 6.5 m homestead fits {}", plot.id);
        }
        let mut cabin = HomeDesign::built_in("cabin").expect("the shipped cabin");
        for plot in ship.plots.iter().filter(|p| p.kind == "cabin") {
            assert_eq!(plot.size.1, 3.0, "{} stays one storey", plot.id);
            assert!(design_fits_plot(&cabin, plot).is_ok(), "the shipped cabin fits {}", plot.id);
            cabin.body.height = 6.0;
            assert!(design_fits_plot(&cabin, plot).is_err(), "a 6 m cabin fits {}", plot.id);
            cabin.body.height = 3.0;
        }
        // The same file with the plots as they were before increment 7 is another ship.
        let mut before = ship.clone();
        for p in before.plots.iter_mut().filter(|p| p.kind == "homestead") {
            p.size.1 = 3.0;
        }
        assert_ne!(before.ship_hash(), ship.ship_hash(), "an older game's 3 m plots are another ship");
    }

    /// AN OLDER GAME IS REFUSED AS ANOTHER SHIP (ship homes increment 7): growing the homestead
    /// plots changed the ship's hash, and the game's welcome check (engine/home_plot.rs
    /// `plan_welcome`) refuses any relay whose hash is not its own, in the one plain sentence,
    /// without giving up a plot. Both ways round: a game built before the plots grew (its ship is
    /// this one with 3 m homestead plots, whose hash is the Wave 0 pin, the hash of the ship file
    /// before increment 7) meeting a relay running this file, and this game meeting an older relay.
    /// The same ship is never refused as another.
    ///
    /// Proven green 2026-10-20 (the existing behaviour, held with the new hash): the refusal does
    /// not depend on which way the hash moved.
    #[cfg(feature = "native")]
    #[test]
    fn an_older_game_is_refused_as_another_ship() {
        use crate::engine::home_plot::{plan_welcome, WelcomeContext, WelcomeHome};
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let now = ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
        let mut older = now.clone();
        for p in older.plots.iter_mut().filter(|p| p.kind == "homestead") {
            p.size.1 = 3.0;
        }
        assert_eq!(older.ship_hash(), "effbdfbec3360115", "the older game's ship is the ship file before increment 7");
        let welcome = |hash: &str| serde_json::json!({ "type": "game_welcome", "player_id": 7, "world_snapshot": [], "home_plot": null, "ship": { "id": now.id, "hash": hash }, "rejoin": false });
        let ctx = WelcomeContext { arrived_on: None, server: "http://127.0.0.1:3210", camera: Vec3::new(53.5, 1.7, 40.5), data_dir: None, other_kind: None };
        for (what, game, relay) in [("an older game, this relay", &older, now.ship_hash()), ("this game, an older relay", &now, older.ship_hash())] {
            match plan_welcome(Some(game), &welcome(&relay), &ctx) {
                WelcomeHome::Refuse { sentence, give_up_plot } => {
                    assert_eq!(sentence, OTHER_SHIP_SENTENCE, "{what}");
                    assert!(!give_up_plot, "{what}: a plot that is not ours to give");
                }
                other => panic!("{what}: another ship must be refused: {other:?}"),
            }
        }
        assert!(!matches!(plan_welcome(Some(&now), &welcome(&now.ship_hash()), &ctx), WelcomeHome::Refuse { .. }), "the same ship is not refused");
    }
}
