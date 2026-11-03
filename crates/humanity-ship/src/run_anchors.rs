//! Anchors for runs, as data: `data/piping/anchors.ron` (2026-11-02, spline routing rung 0,
//! `docs/design/spline-routing.md` sections 5.3 and 12).
//!
//! What holds a run to the structure: straps, staples, cushioned clamps, rod hangers sized by pipe
//! size (UFGS 23 05 15, finding R9), J-hooks, cable hangers, trays, harness ties, riser clamps, and
//! the protector ramps vehicles drive over. One anchor may hold several runs. Nothing places them
//! yet: rung 7 does, and the support rules of `data/piping/run_rules.ron` read them at rung 8.
//!
//! Read from the data folder first, the built-in copy when that is missing or does not parse (the
//! log says so). Plain serde, no `native` gate.

use crate::run_rules::Applies;
use humanity_core::row_source::{self, Findings, Source};
use humanity_core::run_materials::RunMaterials;
use serde::Deserialize;

/// What an anchor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AnchorKind {
    Strap,
    Clip,
    /// A clamp lined so it does not cut or crush plastic pipe and lets it move.
    CushionClamp,
    /// A clevis or ring on a threaded rod from the ceiling.
    RodHanger,
    JHook,
    /// A tie along a cable bundle.
    Tie,
    Clamp,
    Tray,
    /// A protector ramp over runs laid on a floor, for wheels.
    Ramp,
    RiserClamp,
}

/// What an anchor fastens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FixesTo {
    Wood,
    Concrete,
    /// Steel framing: a ship's frames, a steel stud.
    Steel,
    /// Another run (a tie on a bundle).
    Run,
}

/// How an anchor is drawn, metres.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorMesh {
    /// Across the run.
    pub width: f32,
    /// Along the run.
    pub depth: f32,
    pub thickness: f32,
}

/// One anchor row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub id: String,
    pub name: String,
    pub kind: AnchorKind,
    /// Which runs it holds.
    #[serde(default)]
    pub holds: Applies,
    pub fixes_to: Vec<FixesTo>,
    /// The gap it adds between the surface and the run, beyond the run's own radius, metres.
    pub standoff_m: f32,
    /// The most it may carry, newtons; absent where no maker's rating was read.
    #[serde(default)]
    pub load_n: Option<f32>,
    /// How many runs at most.
    #[serde(default)]
    pub holds_max: Option<u32>,
    /// A rod hanger's rod diameter, metres.
    #[serde(default)]
    pub rod_m: Option<f32>,
    pub mesh: AnchorMesh,
    pub source: Vec<Source>,
    #[serde(default)]
    pub note: String,
}

/// The whole file: data/piping/anchors.ron.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchors {
    pub anchors: Vec<Anchor>,
}

impl Anchors {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn anchor(&self, id: &str) -> Option<&Anchor> {
        self.anchors.iter().find(|a| a.id == id)
    }

    pub fn sourced_rows(&self) -> Vec<(String, &[Source])> {
        self.anchors.iter().map(|a| (format!("anchor `{}`", a.id), &a.source[..])).collect()
    }

    /// Everything wrong with the file against the catalog, as sentences (empty = sound).
    pub fn problems(&self, catalog: &RunMaterials) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for a in &self.anchors {
            let row = format!("anchor `{}`", a.id);
            if !ids.insert(a.id.as_str()) {
                out.push(format!("{row} is defined twice"));
            }
            for name in a.holds.unknown_names(catalog) {
                out.push(format!("{row} holds {name}, which the catalog does not declare"));
            }
            if a.fixes_to.is_empty() {
                out.push(format!("{row} fixes to nothing"));
            }
            if a.standoff_m < 0.0 {
                out.push(format!("{row}: a negative stand-off"));
            }
            for v in a.load_n.iter().chain(&a.rod_m) {
                if !(*v > 0.0) {
                    out.push(format!("{row}: a load or rod size must be over 0"));
                }
            }
            if a.holds_max == Some(0) {
                out.push(format!("{row} holds at most nothing"));
            }
            if (a.kind == AnchorKind::RodHanger) != a.rod_m.is_some() {
                out.push(format!("{row}: a rod hanger, and only a rod hanger, has a rod size"));
            }
            if !(a.mesh.width > 0.0 && a.mesh.depth > 0.0 && a.mesh.thickness > 0.0) {
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
const SHIPPED_ANCHORS: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/anchors.ron"));

/// The anchors, loaded once: the data folder's copy first, the shipped copy if that is missing or
/// does not parse (and the log says so, BUG-133).
pub fn anchors() -> &'static Anchors {
    static REG: std::sync::OnceLock<Anchors> = std::sync::OnceLock::new();
    REG.get_or_init(load_anchors)
}

fn load_anchors() -> Anchors {
    let path = humanity_core::storage::data_dir().join("piping").join("anchors.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match Anchors::parse(&text) {
            Ok(a) => return a,
            Err(e) => humanity_core::embedded_data::note_builtin_copy(
                "piping/anchors.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => humanity_core::embedded_data::note_builtin_copy(
            "piping/anchors.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    Anchors::parse(SHIPPED_ANCHORS).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/anchors.ron does not parse: {e}");
        Anchors { anchors: Vec::new() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED_CATALOG: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));

    #[test]
    fn shipped_anchors_registry_parses_and_is_sound() {
        let anchors = Anchors::parse(SHIPPED_ANCHORS).expect("the shipped anchors.ron parses");
        let catalog = RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped catalog parses");
        assert_eq!(anchors.problems(&catalog), Vec::<String>::new());
        let findings = Findings::from_repo(std::path::Path::new(env!("HUMANITY_REPO_ROOT"))).expect("the findings documents");
        assert_eq!(anchors.source_problems(&findings), Vec::<String>::new());
        // UFGS 23 05 15's rod sizes by pipe size (R9): 3/8 in rod for 2 in pipe and smaller.
        let rod = anchors.anchor("rod_hanger_3_8").expect("the 3/8 in rod hanger");
        assert!((rod.rod_m.unwrap() - 0.009525).abs() < 1e-6);
        assert_eq!(rod.holds.dn, Some((0, 50)));
        // A J-hook holds at most 20 cables (R18).
        assert_eq!(anchors.anchor("j_hook").and_then(|j| j.holds_max), Some(20));
    }
}
