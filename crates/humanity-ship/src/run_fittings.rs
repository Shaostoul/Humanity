//! Fittings for runs, as data: `data/piping/fittings.ron` (2026-11-02, spline routing rung 0,
//! `docs/design/spline-routing.md` sections 5.3 and 6.2).
//!
//! Elbows and drain bends, tees, wyes, couplings, unions, reducers, caps, sleeves, glands,
//! transits, bushings, stud plates and PEX bend supports: the angles each turns, its centreline
//! radius, socket depth, friction as an equivalent length, and how it is drawn. A number no source
//! gave is absent, never zero. Nothing places them yet: the curve core (rung 3) turns a rigid
//! corner with one and the mesh (rung 4) draws them. Today's `ship::conduits::FittingKind`
//! (bracket, elbow, passthrough) is what the current router draws and stays until those rungs.
//!
//! Read from the data folder first, the built-in copy when that is missing or does not parse (the
//! log says so). Plain serde, no `native` gate.

use humanity_core::row_source::{self, Findings, Source};
use humanity_core::run_materials::{BendMethod, RunMaterials};
use serde::Deserialize;

/// What a fitting is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FittingKind {
    /// A pressure elbow.
    Elbow,
    /// A drain bend (quarter, sixth, eighth, sixteenth, sweeps).
    DrainBend,
    Tee,
    Wye,
    Coupling,
    Union,
    Reducer,
    Cap,
    /// A sleeve with a gasket through a wall.
    Sleeve,
    /// A watertight cable gland.
    Gland,
    /// A multi-cable transit.
    Transit,
    Bushing,
    /// A steel protection plate over a stud.
    Plate,
    /// A snap-on support that holds a flexible tube at its bend radius in place of an elbow.
    BendSupport,
}

/// A fitting's centreline radius.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Centreline {
    /// A multiple of the run's outside diameter.
    TimesOd(f32),
    /// Metres.
    Metres(f32),
    /// The run's own minimum for that method (a PEX bend support holds the free bend's radius).
    SizeRow(BendMethod),
}

/// How a fitting is drawn, in multiples of the run's outside diameter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FittingMesh {
    /// Its outside diameter.
    pub body: f32,
    /// Its length along the run.
    pub length: f32,
}

/// One fitting row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fitting {
    pub id: String,
    pub name: String,
    pub kind: FittingKind,
    /// The materials it is made for; empty: any run.
    pub materials: Vec<String>,
    /// The turns it makes, degrees; empty for a straight piece.
    pub angles_deg: Vec<f32>,
    #[serde(default)]
    pub centreline: Option<Centreline>,
    #[serde(default)]
    pub socket_m: Option<f32>,
    #[serde(default)]
    pub equivalent_m: Option<f32>,
    pub mesh: FittingMesh,
    pub source: Vec<Source>,
    #[serde(default)]
    pub note: String,
}

/// The whole file: data/piping/fittings.ron.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fittings {
    pub fittings: Vec<Fitting>,
}

impl Fittings {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn fitting(&self, id: &str) -> Option<&Fitting> {
        self.fittings.iter().find(|f| f.id == id)
    }

    /// The fittings made for a material that turn exactly `angle_deg`.
    pub fn turning<'a>(&'a self, material: &'a str, angle_deg: f32) -> impl Iterator<Item = &'a Fitting> + 'a {
        self.fittings.iter().filter(move |f| {
            (f.materials.is_empty() || f.materials.iter().any(|m| m == material))
                && f.angles_deg.iter().any(|a| (a - angle_deg).abs() < 1e-3)
        })
    }

    pub fn sourced_rows(&self) -> Vec<(String, &[Source])> {
        self.fittings.iter().map(|f| (format!("fitting `{}`", f.id), &f.source[..])).collect()
    }

    /// Everything wrong with the file against the catalog, as sentences (empty = sound).
    pub fn problems(&self, catalog: &RunMaterials) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for f in &self.fittings {
            let row = format!("fitting `{}`", f.id);
            if !ids.insert(f.id.as_str()) {
                out.push(format!("{row} is defined twice"));
            }
            for m in f.materials.iter().filter(|m| catalog.material(m).is_none()) {
                out.push(format!("{row} names material `{m}`, which the catalog does not have"));
            }
            for a in &f.angles_deg {
                if !(*a > 0.0 && *a <= 180.0) {
                    out.push(format!("{row}: an angle of {a} degrees"));
                }
            }
            if matches!(f.kind, FittingKind::Elbow | FittingKind::DrainBend | FittingKind::BendSupport) && f.angles_deg.is_empty() {
                out.push(format!("{row}: a bend that turns no angle"));
            }
            match f.centreline {
                Some(Centreline::TimesOd(x)) | Some(Centreline::Metres(x)) if !(x > 0.0) => {
                    out.push(format!("{row}: its centreline radius must be over 0"));
                }
                _ => {}
            }
            for v in f.socket_m.iter().chain(&f.equivalent_m) {
                if !(*v > 0.0) {
                    out.push(format!("{row}: a socket or equivalent length must be over 0"));
                }
            }
            if !(f.mesh.body > 0.0 && f.mesh.length > 0.0) {
                out.push(format!("{row}: its mesh sizes must be over 0"));
            }
        }
        for (row, sources) in self.sourced_rows() {
            out.extend(row_source::shape_problems(&row, sources));
        }
        out
    }

    pub fn source_problems(&self, findings: &Findings) -> Vec<String> {
        self.sourced_rows().into_iter().flat_map(|(row, sources)| row_source::problems(&row, sources, findings)).collect()
    }
}

/// The shipped file, the fallback when the data folder's copy is missing or does not parse.
const SHIPPED_FITTINGS: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/fittings.ron"));

/// The fittings, loaded once: the data folder's copy first, the shipped copy if that is missing or
/// does not parse (and the log says so, BUG-133).
pub fn fittings() -> &'static Fittings {
    static REG: std::sync::OnceLock<Fittings> = std::sync::OnceLock::new();
    REG.get_or_init(load_fittings)
}

fn load_fittings() -> Fittings {
    let path = humanity_core::storage::data_dir().join("piping").join("fittings.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match Fittings::parse(&text) {
            Ok(f) => return f,
            Err(e) => humanity_core::embedded_data::note_builtin_copy(
                "piping/fittings.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => humanity_core::embedded_data::note_builtin_copy(
            "piping/fittings.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    Fittings::parse(SHIPPED_FITTINGS).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/fittings.ron does not parse: {e}");
        Fittings { fittings: Vec::new() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED_CATALOG: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));

    #[test]
    fn shipped_fittings_registry_parses_and_is_sound() {
        let fittings = Fittings::parse(SHIPPED_FITTINGS).expect("the shipped fittings.ron parses");
        let catalog = RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped catalog parses");
        assert_eq!(fittings.problems(&catalog), Vec::<String>::new());
        let findings = Findings::from_repo(std::path::Path::new(env!("HUMANITY_REPO_ROOT"))).expect("the findings documents");
        assert_eq!(fittings.source_problems(&findings), Vec::<String>::new());
        // The drain bends R38 names, at the angles design 6.2 gives them.
        for (id, angle) in [("quarter_bend", 90.0), ("sixth_bend", 60.0), ("eighth_bend", 45.0), ("sixteenth_bend", 22.5)] {
            let f = fittings.fitting(id).unwrap_or_else(|| panic!("the drain bend `{id}`"));
            assert_eq!(f.angles_deg, vec![angle], "{id}");
            assert_eq!(f.source, vec![Source::R(38)], "{id}");
        }
        // Pressure copper turns by 90 and 45-degree elbows.
        assert!(fittings.turning("copper_l_drawn", 90.0).any(|f| f.kind == FittingKind::Elbow));
        assert!(fittings.turning("copper_l_drawn", 45.0).any(|f| f.kind == FittingKind::Elbow));
    }
}
