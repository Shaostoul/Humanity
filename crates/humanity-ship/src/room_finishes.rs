//! ROOM FINISHES, rungs 1 and 2 (docs/design/room-finishes.md): the catalog of trim profiles
//! (skirting, crown, casing, window sill, chair rail, picture rail) and trim materials
//! (data/blueprints/room_finishes.ron), the choice a room or a wall face makes from it
//! ([`Finish`]), and how a face's trim is resolved: the wall face's own override, then the room's
//! choice, then the room purpose's default, then the catalog's fallback, field by field, so an
//! override can change only the material and keep the room's profiles.
//!
//! Where a choice is stored: a room's on its zone (`Zone::finish`, the zone is the room), an
//! interior wall face's on its wall (`InteriorWall::finish`, one per side), an outer wall's inside
//! face on the home (`HomeStructure::shell_finish`, one per edge). Each is left out of the file
//! while empty, so every home written before finishes is written exactly as before. What is drawn
//! from them is crates/humanity-ship/src/room_trim.rs and room_casings.rs.

use serde::{Deserialize, Serialize};

/// What a profile is for: where it is swept and which way its `up` axis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileKind {
    /// Along the foot of a wall, `up` from the floor.
    Skirting,
    /// Along the top of a wall where it meets the ceiling, `up` measured DOWN from the ceiling.
    Crown,
    /// Round a door or a window (rung 2), `up` across the board from the opening's edge (0) to its
    /// outer edge; under a window's stool it is the apron, `up` down from the stool.
    Casing,
    /// Across the wall at about 0.9 m (rung 2), `up` from the rail's underside.
    ChairRail,
    /// Across the wall a little under the ceiling (rung 2), `up` from the rail's underside.
    PictureRail,
    /// A window's stool (the inside sill, rung 2), `up` from its underside to its top, which is
    /// level with the opening's foot.
    Sill,
}

/// One trim cross-section: its visible face as (out, up) metres, from where it meets the wall to
/// where it meets the floor (skirting) or the ceiling (crown), or, for a board fixed to the wall
/// along both edges (a casing, a rail, a stool), from the wall at one edge round to the wall at
/// the other.
#[derive(Debug, Clone, Deserialize)]
pub struct TrimProfile {
    pub id: String,
    pub kind: ProfileKind,
    pub points: Vec<(f32, f32)>,
}

impl TrimProfile {
    /// How far it stands from the wall at most (its depth), metres.
    pub fn depth(&self) -> f32 {
        self.points.iter().map(|p| p.0).fold(0.0, f32::max)
    }

    /// Its extent along `up` (a skirting's height, a crown's drop, a casing's width), metres.
    pub fn height(&self) -> f32 {
        self.points.iter().map(|p| p.1).fold(0.0, f32::max)
    }

    /// It is fixed to the wall along both edges (its outline ends back on the wall): a casing, a
    /// rail, a stool. A skirting and a crown end on the floor or the ceiling instead.
    pub fn closed(&self) -> bool {
        self.points.last().is_some_and(|p| p.0.abs() < 1e-6) && self.points.len() > 2
    }

    /// The same profile at low detail, for homes drawn far off (a neighbour's): its bounding
    /// outline, the top and the face only (and the underside of a board fixed along both edges),
    /// two or three quads a run where the full one has a dozen.
    pub fn coarse(&self) -> Vec<(f32, f32)> {
        let (d, h) = (self.depth(), self.height());
        if self.closed() {
            vec![(0.0, h), (d, h), (d, 0.0), (0.0, 0.0)]
        } else {
            vec![(0.0, h), (d, h), (d, 0.0)]
        }
    }
}

/// Where the trim that is not tied to a floor, a ceiling or an opening goes, and how a window's
/// stool runs past its casing: real fitting measurements, as data (room_finishes.ron `fit`).
#[derive(Debug, Clone, Deserialize)]
pub struct TrimFit {
    /// The height of a chair (dado) rail's centre above the floor, metres.
    pub chair_rail_centre_m: f32,
    /// How far under the ceiling a picture rail's top sits, metres.
    pub picture_rail_below_ceiling_m: f32,
    /// How far a window's stool runs past the casing's outer edge at each end (its horns), metres.
    pub sill_horn_m: f32,
}

impl Default for TrimFit {
    fn default() -> Self {
        TrimFit { chair_rail_centre_m: 0.9, picture_rail_below_ceiling_m: 0.3, sill_horn_m: 0.02 }
    }
}

/// How the shader draws a trim material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrimLookKind {
    /// The colour at the material's roughness (enamel, rubber).
    Paint,
    /// The procedural wood grain (the shader's material type 3).
    Wood,
    /// Brushed metal over the colour (type 1).
    Metal,
}

/// One trim material from the catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct TrimMaterial {
    pub id: String,
    pub look: TrimLookKind,
    pub color: (f32, f32, f32),
    pub roughness: f32,
    pub metallic: f32,
}

/// How one trim group is drawn: (rgba, metallic, roughness, shader material type). What the home's
/// trim family fills its slot's material with (engine/home_meshes.rs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrimLook {
    pub rgba: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub material_type: f32,
}

impl TrimMaterial {
    /// The look it is drawn in. A Paint is the shader's type 0, which lays the wall PANEL GRID over
    /// any surface with metallic under 0.1 and roughness over 0.3
    /// (assets/shaders/pbr/90-fragment-main.wgsl, "Type 0"); a painted board has no grid, so a
    /// rough paint (rubber) is drawn at metallic 0.1, the walls' own escape from the grid.
    pub fn trim_look(&self) -> TrimLook {
        let rgba = [self.color.0, self.color.1, self.color.2, 1.0];
        let (metallic, ty) = match self.look {
            TrimLookKind::Paint if self.roughness > 0.3 && self.metallic < 0.1 => (0.1, 0.0),
            TrimLookKind::Paint => (self.metallic, 0.0),
            TrimLookKind::Wood => (0.0, 3.0),
            TrimLookKind::Metal => (self.metallic, 1.0),
        };
        TrimLook { rgba, metallic, roughness: self.roughness, material_type: ty }
    }
}

/// A finish CHOICE, a room's or one wall face's: every field None inherits (the face from the
/// room, the room from its purpose's default). A profile id, or "none" for no trim of that kind.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Finish {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skirting: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crown: Option<String>,
    /// A trim material id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// The casing round each door and window on this face (rung 2), and a window's apron.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub casing: Option<String>,
    /// A window's stool (rung 2); "none" frames the window on all four sides instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chair_rail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture_rail: Option<String>,
}

impl Finish {
    /// Serde's "leave it out" test: a choice that chooses nothing is not written.
    pub fn is_empty(&self) -> bool {
        self.ids().iter().all(|f| f.is_none()) && self.material.is_none()
    }

    /// The profile fields with the kind each picks, in the editor's order.
    pub fn ids(&self) -> [&Option<String>; 6] {
        [&self.skirting, &self.crown, &self.casing, &self.sill, &self.chair_rail, &self.picture_rail]
    }

    /// The profile field that picks a profile of `kind`, to change.
    pub fn field_mut(&mut self, kind: ProfileKind) -> &mut Option<String> {
        match kind {
            ProfileKind::Skirting => &mut self.skirting,
            ProfileKind::Crown => &mut self.crown,
            ProfileKind::Casing => &mut self.casing,
            ProfileKind::Sill => &mut self.sill,
            ProfileKind::ChairRail => &mut self.chair_rail,
            ProfileKind::PictureRail => &mut self.picture_rail,
        }
    }

    /// The profile field that picks a profile of `kind`.
    pub fn field(&self, kind: ProfileKind) -> &Option<String> {
        match kind {
            ProfileKind::Skirting => &self.skirting,
            ProfileKind::Crown => &self.crown,
            ProfileKind::Casing => &self.casing,
            ProfileKind::Sill => &self.sill,
            ProfileKind::ChairRail => &self.chair_rail,
            ProfileKind::PictureRail => &self.picture_rail,
        }
    }

    /// This choice with every field it leaves open taken from `under`.
    pub fn over(&self, under: &Finish) -> Finish {
        let pick = |a: &Option<String>, b: &Option<String>| a.clone().or_else(|| b.clone());
        Finish {
            skirting: pick(&self.skirting, &under.skirting),
            crown: pick(&self.crown, &under.crown),
            material: pick(&self.material, &under.material),
            casing: pick(&self.casing, &under.casing),
            sill: pick(&self.sill, &under.sill),
            chair_rail: pick(&self.chair_rail, &under.chair_rail),
            picture_rail: pick(&self.picture_rail, &under.picture_rail),
        }
    }
}

/// Every profile kind a finish picks, in the editor's order.
pub const FINISH_KINDS: [ProfileKind; 6] = [ProfileKind::Skirting, ProfileKind::Crown, ProfileKind::Casing, ProfileKind::Sill, ProfileKind::ChairRail, ProfileKind::PictureRail];

/// A wall's own finish, one choice per side: `left` is the side on your left walking from the
/// wall's `a` to its `b` seen from above (x east, z south), `right` the other.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WallFinish {
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub left: Finish,
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub right: Finish,
}

impl WallFinish {
    /// Serde's "leave it out" test.
    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    /// The side's choice (`left` true for the left side).
    pub fn side(&self, left: bool) -> &Finish {
        if left {
            &self.left
        } else {
            &self.right
        }
    }

    /// The side's choice, to change.
    pub fn side_mut(&mut self, left: bool) -> &mut Finish {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }
}

/// THE PERIMETER SHELL'S OWN FINISH (rung 2, the plan's per-edge finish): one choice for the inside
/// face of each of the box's four outer walls, as an interior wall's side has its own. Named by
/// compass, x east and z south: `north` is the z = 0 edge (perimeter edge 0), `east` x = width
/// (1), `south` z = depth (2), `west` x = 0 (3). Left out of the file while empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShellFinish {
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub north: Finish,
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub east: Finish,
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub south: Finish,
    #[serde(default, skip_serializing_if = "Finish::is_empty")]
    pub west: Finish,
}

impl ShellFinish {
    /// Serde's "leave it out" test.
    pub fn is_empty(&self) -> bool {
        [&self.north, &self.east, &self.south, &self.west].iter().all(|f| f.is_empty())
    }

    /// Perimeter edge `edge`'s choice (0 north, 1 east, 2 south, 3 west: the shell's build order).
    pub fn edge(&self, edge: usize) -> &Finish {
        match edge % 4 {
            0 => &self.north,
            1 => &self.east,
            2 => &self.south,
            _ => &self.west,
        }
    }

    /// Perimeter edge `edge`'s choice, to change.
    pub fn edge_mut(&mut self, edge: usize) -> &mut Finish {
        match edge % 4 {
            0 => &mut self.north,
            1 => &mut self.east,
            2 => &mut self.south,
            _ => &mut self.west,
        }
    }
}

fn none_id() -> String {
    NONE_ID.to_string()
}

/// One row of room-purpose defaults. A rung 2 field a row leaves out is "none".
#[derive(Debug, Clone, Deserialize)]
pub struct RoomTypeDefault {
    pub room_types: Vec<String>,
    pub skirting: String,
    pub crown: String,
    pub material: String,
    #[serde(default = "none_id")]
    pub casing: String,
    #[serde(default = "none_id")]
    pub sill: String,
    #[serde(default = "none_id")]
    pub chair_rail: String,
    #[serde(default = "none_id")]
    pub picture_rail: String,
}

/// The fallback row (a room no default row names, or a room with no zone).
#[derive(Debug, Clone, Deserialize)]
pub struct FinishFallback {
    pub skirting: String,
    pub crown: String,
    pub material: String,
    #[serde(default = "none_id")]
    pub casing: String,
    #[serde(default = "none_id")]
    pub sill: String,
    #[serde(default = "none_id")]
    pub chair_rail: String,
    #[serde(default = "none_id")]
    pub picture_rail: String,
}

/// The whole catalog, data/blueprints/room_finishes.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct RoomFinishCatalog {
    pub profiles: Vec<TrimProfile>,
    pub materials: Vec<TrimMaterial>,
    #[serde(default)]
    pub fit: TrimFit,
    pub by_room_type: Vec<RoomTypeDefault>,
    pub fallback: FinishFallback,
}

/// The finish a face is drawn with once every level has spoken: the profiles (None: no trim of
/// that kind) and the material.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedFinish<'a> {
    pub skirting: Option<&'a TrimProfile>,
    pub crown: Option<&'a TrimProfile>,
    pub casing: Option<&'a TrimProfile>,
    pub sill: Option<&'a TrimProfile>,
    pub chair_rail: Option<&'a TrimProfile>,
    pub picture_rail: Option<&'a TrimProfile>,
    pub material: &'a TrimMaterial,
}

/// The id that means "no trim of this kind".
pub const NONE_ID: &str = "none";

impl RoomFinishCatalog {
    /// Parse a catalog from RON text.
    pub fn parse(src: &str) -> Result<Self, String> {
        let mut cat: RoomFinishCatalog = ron::from_str(src).map_err(|e| format!("room_finishes.ron: {e}"))?;
        // Drop a point that repeats the one before it: a zero-length segment has no normal.
        for p in &mut cat.profiles {
            p.points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
        }
        Ok(cat)
    }

    /// A profile by id.
    pub fn profile(&self, id: &str) -> Option<&TrimProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// A material by id.
    pub fn material(&self, id: &str) -> Option<&TrimMaterial> {
        self.materials.iter().find(|m| m.id == id)
    }

    /// The profiles of one kind, in catalog order (the editor's picker).
    pub fn profiles_of(&self, kind: ProfileKind) -> impl Iterator<Item = &TrimProfile> {
        self.profiles.iter().filter(move |p| p.kind == kind)
    }

    /// The default finish of a room of purpose `room_type` (a data/rooms.ron key; None for a room
    /// with no purpose): its row, or the fallback.
    pub fn default_for(&self, room_type: Option<&str>) -> Finish {
        let row = room_type.and_then(|rt| self.by_room_type.iter().find(|r| r.room_types.iter().any(|t| t == rt)));
        let fb = &self.fallback;
        let ids: [&String; 7] = match row {
            Some(r) => [&r.skirting, &r.crown, &r.material, &r.casing, &r.sill, &r.chair_rail, &r.picture_rail],
            None => [&fb.skirting, &fb.crown, &fb.material, &fb.casing, &fb.sill, &fb.chair_rail, &fb.picture_rail],
        };
        let s = |i: usize| Some(ids[i].clone());
        Finish { skirting: s(0), crown: s(1), material: s(2), casing: s(3), sill: s(4), chair_rail: s(5), picture_rail: s(6) }
    }

    /// The fallback's profile id of `kind`.
    fn fallback_id(&self, kind: ProfileKind) -> &str {
        let fb = &self.fallback;
        match kind {
            ProfileKind::Skirting => &fb.skirting,
            ProfileKind::Crown => &fb.crown,
            ProfileKind::Casing => &fb.casing,
            ProfileKind::Sill => &fb.sill,
            ProfileKind::ChairRail => &fb.chair_rail,
            ProfileKind::PictureRail => &fb.picture_rail,
        }
    }

    /// Resolve a face's finish: `face` (the wall side's own choice) over `room` (the zone's) over
    /// the room purpose's default. An id the catalog does not know falls to the fallback's, so a
    /// typo draws the plain default rather than nothing (and `unknown_ids` names it).
    pub fn resolve(&self, face: Option<&Finish>, room: Option<&Finish>, room_type: Option<&str>) -> ResolvedFinish<'_> {
        let base = self.default_for(room_type);
        let empty = Finish::default();
        let f = face.unwrap_or(&empty).over(&room.unwrap_or(&empty).over(&base));
        let pick = |kind: ProfileKind| -> Option<&TrimProfile> {
            let fallback = self.fallback_id(kind);
            let id = f.field(kind).as_deref().unwrap_or(fallback);
            if id == NONE_ID {
                return None;
            }
            match self.profile(id) {
                Some(p) if p.kind == kind => Some(p),
                _ if fallback == NONE_ID => None,
                _ => self.profile(fallback).filter(|p| p.kind == kind),
            }
        };
        let material = f
            .material
            .as_deref()
            .and_then(|id| self.material(id))
            .or_else(|| self.material(&self.fallback.material))
            .or_else(|| self.materials.first())
            .expect("the room finish catalog has at least one material");
        ResolvedFinish {
            skirting: pick(ProfileKind::Skirting),
            crown: pick(ProfileKind::Crown),
            casing: pick(ProfileKind::Casing),
            sill: pick(ProfileKind::Sill),
            chair_rail: pick(ProfileKind::ChairRail),
            picture_rail: pick(ProfileKind::PictureRail),
            material,
        }
    }

    /// Every id a choice names that the catalog does not have (for the editor's warning line).
    pub fn unknown_ids(&self, f: &Finish) -> Vec<String> {
        let mut out = Vec::new();
        for id in f.ids().into_iter().flatten() {
            if id != NONE_ID && self.profile(id).is_none() {
                out.push(id.clone());
            }
        }
        if let Some(m) = &f.material {
            if self.material(m).is_none() {
                out.push(m.clone());
            }
        }
        out
    }
}

/// A profile's whole cross-section as a closed outline: its points, and for a skirting or a crown
/// (whose outline ends on the floor or the ceiling, not back on the wall) the wall corner (0, 0)
/// first.
pub fn closed_section(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut section = pts.to_vec();
    if section.last().is_some_and(|p| p.0.abs() > 1e-6) {
        section.insert(0, (0.0, 0.0));
    }
    section
}

/// The triangles filling a simple polygon (ear clipping, any winding): a trim's cross-section, which
/// is not convex for a beaded skirting or a picture rail's hook. The bake's end caps and the
/// editor's drawing both fill a section with it.
pub fn triangulate(poly: &[(f32, f32)]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n).map(|i| poly[i].0 * poly[(i + 1) % n].1 - poly[(i + 1) % n].0 * poly[i].1).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let c = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| ((b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)) * sign;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (a, b, q) = (poly[idx[(i + m - 1) % m]], poly[idx[i]], poly[idx[(i + 1) % m]]);
            c(a, b, q) > 1e-12 && idx.iter().all(|&j| [idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]].contains(&j) || !(c(a, b, poly[j]) > 0.0 && c(b, q, poly[j]) > 0.0 && c(q, a, poly[j]) > 0.0))
        });
        // No clean ear (a degenerate outline): clip the first vertex, as a fan would.
        let i = ear.unwrap_or(0);
        out.push([idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]]);
        idx.remove(i);
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// The catalog, parsed once from the copy built into the program (like `wall_materials`, so the
/// editor and the bake read one list with no path to find at run time). Static data, not a cache:
/// it cannot go stale within a run because the text it is parsed from is compiled in.
pub fn catalog() -> &'static RoomFinishCatalog {
    static CAT: std::sync::OnceLock<RoomFinishCatalog> = std::sync::OnceLock::new();
    CAT.get_or_init(|| {
        const SRC: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/blueprints/room_finishes.ron"));
        RoomFinishCatalog::parse(SRC).unwrap_or_else(|e| {
            log::error!("{e}");
            RoomFinishCatalog {
                profiles: Vec::new(),
                materials: vec![TrimMaterial { id: "white_enamel".into(), look: TrimLookKind::Paint, color: (0.86, 0.85, 0.82), roughness: 0.3, metallic: 0.0 }],
                fit: TrimFit::default(),
                by_room_type: Vec::new(),
                fallback: FinishFallback {
                    skirting: none_id(),
                    crown: none_id(),
                    material: "white_enamel".into(),
                    casing: none_id(),
                    sill: none_id(),
                    chair_rail: none_id(),
                    picture_rail: none_id(),
                },
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_catalog_parses_and_every_default_names_real_entries() {
        let src = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/blueprints/room_finishes.ron")).unwrap();
        let cat = RoomFinishCatalog::parse(&src).expect("parses");
        assert!(cat.profiles_of(ProfileKind::Skirting).count() >= 3);
        assert!(cat.profiles_of(ProfileKind::Crown).count() >= 2);
        let check = |id: &str, kind: ProfileKind| assert!(id == NONE_ID || cat.profile(id).is_some_and(|p| p.kind == kind), "{id} is a {kind:?}");
        for k in FINISH_KINDS {
            assert!(cat.profiles_of(k).count() >= 1, "no {k:?} profile");
        }
        for r in &cat.by_room_type {
            check(&r.skirting, ProfileKind::Skirting);
            check(&r.crown, ProfileKind::Crown);
            check(&r.casing, ProfileKind::Casing);
            check(&r.sill, ProfileKind::Sill);
            check(&r.chair_rail, ProfileKind::ChairRail);
            check(&r.picture_rail, ProfileKind::PictureRail);
            assert!(cat.material(&r.material).is_some(), "{}", r.material);
        }
        let fb = &cat.fallback;
        for (id, k) in [(&fb.skirting, ProfileKind::Skirting), (&fb.crown, ProfileKind::Crown), (&fb.casing, ProfileKind::Casing), (&fb.sill, ProfileKind::Sill), (&fb.chair_rail, ProfileKind::ChairRail), (&fb.picture_rail, ProfileKind::PictureRail)] {
            check(id, k);
        }
        for p in &cat.profiles {
            // Each starts on the wall and ends on the floor or ceiling, or, a board fixed along both
            // edges (a casing, a rail, a stool), back on the wall at its other edge.
            let (first, last) = (p.points[0], *p.points.last().unwrap());
            assert!(first.0.abs() < 1e-6, "{} starts on the wall", p.id);
            assert!(last.1.abs() < 1e-6, "{} ends on its mounting surface", p.id);
            assert!(p.depth() > 0.0 && p.height() > 0.0);
            let closed = matches!(p.kind, ProfileKind::Casing | ProfileKind::ChairRail | ProfileKind::PictureRail | ProfileKind::Sill);
            assert_eq!(p.closed(), closed, "{} ends back on the wall exactly when it is a board fixed along both edges", p.id);
        }
    }

    /// A face's override wins over its room's choice, the room's over its purpose's default, field
    /// by field. Red first (2026-11-14): with `resolve` taking the room's material before the
    /// face's (`room.over(face)`), the face's oak came out white enamel.
    #[test]
    fn a_wall_face_override_wins_over_the_room_choice() {
        let cat = catalog();
        let room = Finish { skirting: Some("ogee_170".into()), material: Some("white_enamel".into()), ..Default::default() };
        let face = Finish { material: Some("oak".into()), ..Default::default() };
        let r = cat.resolve(Some(&face), Some(&room), Some("livingroom"));
        assert_eq!(r.material.id, "oak", "the face's material");
        assert_eq!(r.skirting.map(|p| p.id.as_str()), Some("ogee_170"), "the room's skirting, the face leaving it open");
        assert_eq!(r.crown.map(|p| p.id.as_str()), Some("cove_70"), "the living room default's crown");
        // "none" turns a kind off at any level.
        let off = Finish { skirting: Some(NONE_ID.into()), ..Default::default() };
        assert!(cat.resolve(Some(&off), Some(&room), None).skirting.is_none());
        // A purpose with no row, and no purpose, take the fallback.
        let fb = cat.resolve(None, None, Some("not_a_room"));
        assert_eq!(fb.skirting.map(|p| p.id.as_str()), Some(cat.fallback.skirting.as_str()));
        assert!(cat.resolve(None, None, Some("garden")).skirting.is_none(), "a garden has no skirting");
    }

    /// Every name the editor shows is a word in data/i18n/en.json (finishes.profile.<id>,
    /// finishes.material.<id>), so a profile added to the data without its words fails here.
    #[test]
    fn every_catalog_entry_has_its_words() {
        let text = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/i18n/en.json")).unwrap();
        let en: serde_json::Value = serde_json::from_str(&text).unwrap();
        let cat = catalog();
        for p in &cat.profiles {
            assert!(en["finishes"]["profile"][&p.id].is_string(), "en.json lacks finishes.profile.{}", p.id);
        }
        for m in &cat.materials {
            assert!(en["finishes"]["material"][&m.id].is_string(), "en.json lacks finishes.material.{}", m.id);
        }
    }

    #[test]
    fn paint_looks_never_take_the_panel_grid() {
        for m in &catalog().materials {
            let l = m.trim_look();
            if l.material_type == 0.0 {
                assert!(!(l.metallic < 0.1 && l.roughness > 0.3), "{} would draw the wall panel grid", m.id);
            }
        }
    }
}
