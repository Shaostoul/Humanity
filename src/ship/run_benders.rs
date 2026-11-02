//! Benders, as data: `data/piping/benders.ron` (2026-11-02, spline routing rung 0,
//! `docs/design/spline-routing.md` sections 5.3 and 6.2).
//!
//! The tools that bend tube and conduit: which sizes each bends, at what centreline radius, and the
//! angles marked on it (Klein's EMT benders, finding R24; the copper benders at CDA Table 4.1's
//! radii, R1). The curve core (rung 3) fillets a bendable run's corner at its bender's radius; a
//! material and size with no bender here turns with fittings (hard copper over 3/4 in).
//!
//! Read from the data folder first, the built-in copy when that is missing or does not parse (the
//! log says so). Plain serde, no `native` gate.

use humanity_core::row_source::{self, Findings, Source};
use humanity_core::run_materials::{BendMethod, Behaviour, RunMaterials};
use serde::Deserialize;

/// What kind of bender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum BenderKind {
    /// A lever tube bender.
    Lever,
    /// A hand bender: a shoe on a handle (EMT), or a hand tube bender.
    Hand,
    Machine,
}

/// One size a bender bends, and its radius for it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenderSize {
    /// A size row's id in the catalog.
    pub size: String,
    /// The centreline radius it bends that size at, metres.
    pub clr_m: f32,
    /// The source's own number.
    #[serde(default)]
    pub as_read: String,
}

/// One bender row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bender {
    pub id: String,
    pub name: String,
    pub kind: BenderKind,
    pub sizes: Vec<BenderSize>,
    /// The angles marked on it, degrees.
    #[serde(default)]
    pub marks_deg: Vec<f32>,
    /// The angles its angle setter stops at, degrees.
    #[serde(default)]
    pub stops_deg: Vec<f32>,
    pub source: Vec<Source>,
    #[serde(default)]
    pub note: String,
}

/// The whole file: data/piping/benders.ron.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Benders {
    pub benders: Vec<Bender>,
}

impl Benders {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn bender(&self, id: &str) -> Option<&Bender> {
        self.benders.iter().find(|b| b.id == id)
    }

    /// Every bender that bends a size, with its radius for it.
    pub fn for_size<'a>(&'a self, size_id: &'a str) -> impl Iterator<Item = (&'a Bender, f32)> + 'a {
        self.benders.iter().filter_map(move |b| b.sizes.iter().find(|s| s.size == size_id).map(|s| (b, s.clr_m)))
    }

    pub fn sourced_rows(&self) -> Vec<(String, &[Source])> {
        self.benders.iter().map(|b| (format!("bender `{}`", b.id), &b.source[..])).collect()
    }

    /// Everything wrong with the file against the catalog, as sentences (empty = sound): every size
    /// it names is a catalog size of a bendable material, and no bender bends tighter than the size
    /// row's own minimum for a bender, where the row gives one.
    pub fn problems(&self, catalog: &RunMaterials) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for b in &self.benders {
            let row = format!("bender `{}`", b.id);
            if !ids.insert(b.id.as_str()) {
                out.push(format!("{row} is defined twice"));
            }
            if b.sizes.is_empty() {
                out.push(format!("{row} bends no size"));
            }
            for s in &b.sizes {
                match catalog.size(&s.size) {
                    None => out.push(format!("{row} names size `{}`, which the catalog does not have", s.size)),
                    Some((m, size)) => {
                        if m.behaviour != Behaviour::Bendable {
                            out.push(format!("{row} bends `{}`, whose material `{}` is not Bendable", s.size, m.id));
                        }
                        if let Some(min) = size.bend_radius(BendMethod::Bender) {
                            if s.clr_m + 1e-6 < min {
                                out.push(format!("{row} bends `{}` at {} m, under its minimum of {min} m", s.size, s.clr_m));
                            }
                        }
                    }
                }
                if !(s.clr_m > 0.0) {
                    out.push(format!("{row}: a radius must be over 0"));
                }
            }
            for a in b.marks_deg.iter().chain(&b.stops_deg) {
                if !(*a > 0.0 && *a <= 180.0) {
                    out.push(format!("{row}: a mark of {a} degrees"));
                }
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
const SHIPPED_BENDERS: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/benders.ron"));

/// The benders, loaded once: the data folder's copy first, the shipped copy if that is missing or
/// does not parse (and the log says so, BUG-133).
pub fn benders() -> &'static Benders {
    static REG: std::sync::OnceLock<Benders> = std::sync::OnceLock::new();
    REG.get_or_init(load_benders)
}

fn load_benders() -> Benders {
    let path = humanity_core::storage::data_dir().join("piping").join("benders.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match Benders::parse(&text) {
            Ok(b) => return b,
            Err(e) => humanity_core::embedded_data::note_builtin_copy(
                "piping/benders.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => humanity_core::embedded_data::note_builtin_copy(
            "piping/benders.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    Benders::parse(SHIPPED_BENDERS).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/benders.ron does not parse: {e}");
        Benders { benders: Vec::new() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED_CATALOG: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));

    #[test]
    fn shipped_benders_registry_parses_and_is_sound() {
        let benders = Benders::parse(SHIPPED_BENDERS).expect("the shipped benders.ron parses");
        let catalog = RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped catalog parses");
        assert_eq!(benders.problems(&catalog), Vec::<String>::new());
        let findings = Findings::from_repo(std::path::Path::new(env!("HUMANITY_REPO_ROOT"))).expect("the findings documents");
        assert_eq!(benders.source_problems(&findings), Vec::<String>::new());
        // Klein's 1/2 in EMT bender: 4 5/8 in centreline, marks 10 to 60 degrees (R24).
        let (klein, r) = benders.for_size("emt_1_2").next().expect("a bender for 1/2 in EMT");
        assert!((r - 0.117475).abs() < 1e-6, "{r}");
        assert_eq!(klein.marks_deg, vec![10.0, 22.5, 30.0, 45.0, 60.0]);
        // Hard copper over 3/4 in has no bender: it turns with fittings (R1, design 6.2).
        assert!(benders.for_size("copper_l_drawn_1").next().is_none());
    }

    /// A bender tighter than its size's own minimum is caught. Seen red on purpose here.
    #[test]
    fn a_bender_tighter_than_the_size_minimum_is_a_problem() {
        let catalog = RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped catalog parses");
        let bad = SHIPPED_BENDERS.replacen("clr_m: 0.05715", "clr_m: 0.03", 1);
        let problems = Benders::parse(&bad).expect("parses").problems(&catalog);
        assert!(problems.iter().any(|p| p.contains("under its minimum")), "{problems:?}");
    }
}
