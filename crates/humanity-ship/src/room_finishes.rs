//! ROOM FINISHES, rung 1 (docs/design/room-finishes.md): the catalog of trim profiles and trim
//! materials (data/blueprints/room_finishes.ron), the choice a room or a wall face makes from it
//! ([`Finish`]), and how a face's trim is resolved: the wall face's own override, then the room's
//! choice, then the room purpose's default, then the catalog's fallback, field by field, so an
//! override can change only the material and keep the room's profiles.
//!
//! Where a choice is stored: a room's on its zone (`Zone::finish`, the zone is the room), a wall
//! face's on its wall (`InteriorWall::finish`, one per side). Both are left out of the file while
//! empty, so every home written before finishes is written exactly as before. What is drawn from
//! them is crates/humanity-ship/src/room_trim.rs.

use serde::{Deserialize, Serialize};

/// What a profile is for: where it is swept and which way its `up` axis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileKind {
    /// Along the foot of a wall, `up` from the floor.
    Skirting,
    /// Along the top of a wall where it meets the ceiling, `up` measured DOWN from the ceiling.
    Crown,
    /// Round a door or a window (rung 2), `up` across the board from the opening's edge.
    Casing,
}

/// One trim cross-section: its visible face as (out, up) metres, from where it meets the wall to
/// where it meets the floor (skirting) or the ceiling (crown).
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

    /// Its extent along `up` (a skirting's height, a crown's drop), metres.
    pub fn height(&self) -> f32 {
        self.points.iter().map(|p| p.1).fold(0.0, f32::max)
    }

    /// The same profile at low detail, for homes drawn far off (a neighbour's): its bounding
    /// outline, the top and the face only, two quads a run where the full one has a dozen.
    pub fn coarse(&self) -> Vec<(f32, f32)> {
        let (d, h) = (self.depth(), self.height());
        vec![(0.0, h), (d, h), (d, 0.0)]
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
}

impl Finish {
    /// Serde's "leave it out" test: a choice that chooses nothing is not written.
    pub fn is_empty(&self) -> bool {
        self.skirting.is_none() && self.crown.is_none() && self.material.is_none()
    }

    /// This choice with every field it leaves open taken from `under`.
    pub fn over(&self, under: &Finish) -> Finish {
        Finish {
            skirting: self.skirting.clone().or_else(|| under.skirting.clone()),
            crown: self.crown.clone().or_else(|| under.crown.clone()),
            material: self.material.clone().or_else(|| under.material.clone()),
        }
    }
}

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

/// One row of room-purpose defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct RoomTypeDefault {
    pub room_types: Vec<String>,
    pub skirting: String,
    pub crown: String,
    pub material: String,
}

/// The fallback row (a room no default row names, or a room with no zone).
#[derive(Debug, Clone, Deserialize)]
pub struct FinishFallback {
    pub skirting: String,
    pub crown: String,
    pub material: String,
}

/// The whole catalog, data/blueprints/room_finishes.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct RoomFinishCatalog {
    pub profiles: Vec<TrimProfile>,
    pub materials: Vec<TrimMaterial>,
    pub by_room_type: Vec<RoomTypeDefault>,
    pub fallback: FinishFallback,
}

/// The finish a face is drawn with once every level has spoken: the profiles (None: no trim of
/// that kind) and the material.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedFinish<'a> {
    pub skirting: Option<&'a TrimProfile>,
    pub crown: Option<&'a TrimProfile>,
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
        let (s, c, m) = match row {
            Some(r) => (&r.skirting, &r.crown, &r.material),
            None => (&self.fallback.skirting, &self.fallback.crown, &self.fallback.material),
        };
        Finish { skirting: Some(s.clone()), crown: Some(c.clone()), material: Some(m.clone()) }
    }

    /// Resolve a face's finish: `face` (the wall side's own choice) over `room` (the zone's) over
    /// the room purpose's default. An id the catalog does not know falls to the fallback's, so a
    /// typo draws the plain default rather than nothing (and `unknown_ids` names it).
    pub fn resolve(&self, face: Option<&Finish>, room: Option<&Finish>, room_type: Option<&str>) -> ResolvedFinish<'_> {
        let base = self.default_for(room_type);
        let empty = Finish::default();
        let f = face.unwrap_or(&empty).over(&room.unwrap_or(&empty).over(&base));
        let pick = |id: &Option<String>, kind: ProfileKind, fallback: &str| -> Option<&TrimProfile> {
            let id = id.as_deref().unwrap_or(fallback);
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
            skirting: pick(&f.skirting, ProfileKind::Skirting, &self.fallback.skirting),
            crown: pick(&f.crown, ProfileKind::Crown, &self.fallback.crown),
            material,
        }
    }

    /// Every id a choice names that the catalog does not have (for the editor's warning line).
    pub fn unknown_ids(&self, f: &Finish) -> Vec<String> {
        let mut out = Vec::new();
        for id in [&f.skirting, &f.crown].into_iter().flatten() {
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
                by_room_type: Vec::new(),
                fallback: FinishFallback { skirting: NONE_ID.into(), crown: NONE_ID.into(), material: "white_enamel".into() },
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
        for r in &cat.by_room_type {
            check(&r.skirting, ProfileKind::Skirting);
            check(&r.crown, ProfileKind::Crown);
            assert!(cat.material(&r.material).is_some(), "{}", r.material);
        }
        check(&cat.fallback.skirting, ProfileKind::Skirting);
        check(&cat.fallback.crown, ProfileKind::Crown);
        for p in &cat.profiles {
            // Each starts on the wall and ends on the floor or ceiling (or the opening's edge).
            let (first, last) = (p.points[0], *p.points.last().unwrap());
            assert!(first.0.abs() < 1e-6, "{} starts on the wall", p.id);
            assert!(last.1.abs() < 1e-6, "{} ends on its mounting surface", p.id);
            assert!(p.depth() > 0.0 && p.height() > 0.0);
        }
    }

    /// A face's override wins over its room's choice, the room's over its purpose's default, field
    /// by field. Red first (2026-11-14): with `resolve` taking the room's material before the
    /// face's (`room.over(face)`), the face's oak came out white enamel.
    #[test]
    fn a_wall_face_override_wins_over_the_room_choice() {
        let cat = catalog();
        let room = Finish { skirting: Some("ogee_170".into()), crown: None, material: Some("white_enamel".into()) };
        let face = Finish { skirting: None, crown: None, material: Some("oak".into()) };
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
