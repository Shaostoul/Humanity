//! THE ONE CATALOG of what a run is made of: `data/piping/pipe_materials.ron` (2026-10-04, the
//! pipe looks; grown into the one catalog 2026-11-02, spline routing rung 0,
//! `docs/design/spline-routing.md` section 5.3).
//!
//! A "run" is one pipe, hose, tube or cable, the trades' own word (design 5.4). One material id
//! resolves to its look, its behaviour class (Rigid, Bendable, Flexible: design 6.1) and what it
//! may carry; one size id to that size's dimensions, its minimum bend radius by method, and its
//! electrical, fluid or data specs. The cable registry that was `data/utilities/conduits.ron`
//! (v0.604) merged in as size rows of their materials, keeping every id, so a connection's pinned
//! `spec` ("cu_awg12", "pex_half") resolves here.
//!
//! WHY IT LIVES IN THE CORE. `humanity_core::utilities` runs the cable and data checks
//! (`check_cable`, `check_data_link`, the auto-pickers) and needs the cable specs, and the core may
//! reference no crate above it (`scripts/crate-layers.json`), so the catalog's plain-data types and
//! its loader sit here, beside `utilities`, where every crate can read them: the ship's pipe looks
//! (`ship::pipe_materials`), the rules and parts that name its materials (`ship::run_rules` and
//! friends), and later the run model, the curve and the checks (design section 17). Plain serde, no
//! `native` gate: it compiles into the relay too, which will check shared runs.
//!
//! Read from the data folder first (so it can be modded) and from the copy built into the exe when
//! that is missing or does not parse, which the log says (BUG-133), as every disk-first registry
//! here does. Read once per run: an edit applies on the next start.

use crate::row_source::{self, Findings, Source};
use crate::utilities::{ConductorMaterial, Grade, Utility};
use serde::Deserialize;

/// How a material takes a change of direction (design 6.1). A material's class is fixed; a size
/// with no bender row turns with fittings (hard copper over 3/4 in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Behaviour {
    /// Straight lengths joined by fittings of standard angles; never bent in place (PVC, CPVC, steel).
    Rigid,
    /// Straight lengths joined by bends of at least the method's radius, or by fittings (copper
    /// tube, EMT, CSST).
    Bendable,
    /// A smooth curve through its points, hanging or lying where gravity and its stiffness put it
    /// (hose, cord, cable, PEX in free lay).
    Flexible,
}

/// What kind of run a material makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Form {
    /// Any fluid line: pipe, tube or hose.
    Pipe,
    /// Wire, cable or optical fibre.
    Cable,
    /// A conduit or tray that cables are pulled through or laid in.
    Raceway,
}

/// How a run is bent, for a minimum bend radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum BendMethod {
    /// With a bender (a tool of a stated radius: data/piping/benders.ron).
    Bender,
    /// By hand without a tool, at rest in place (PEX's free bend; a cable's installed radius).
    Free,
    /// PEX bent against the set of its coil.
    AgainstCoil,
    /// A cable while it is being pulled, under tension.
    Pulling,
}

/// A construction word a material may carry and a rule may name ("armour", "fibre").
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagRow {
    pub id: String,
    pub meaning: String,
}

/// Which material one of today's four drawn conduit kinds (`ship::conduits::ConduitKind`) is drawn
/// in. A mapping, not a fact, so it carries no source; rung 1 removes it with `ConduitKind`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindMaterial {
    pub kind: String,
    pub material: String,
}

/// How a material's body is drawn.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// sRGB, 0-255: our rendition of a dielectric's colour, or a bare metal's MEASURED reflectance
    /// (the shader takes a metal's F0 straight from its base colour).
    pub srgb: (u8, u8, u8),
    pub metallic: f32,
    pub roughness: f32,
    #[serde(default)]
    pub note: String,
}

impl Look {
    /// The colour in LINEAR light, alpha 1, for a renderer material's base colour (the PBR shader
    /// treats `base_color` as linear albedo).
    pub fn linear_rgba(&self) -> [f32; 4] {
        let (r, g, b) = self.srgb;
        [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0]
    }
}

/// A material's own minimum bend radius for one method.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BendRadius {
    pub method: BendMethod,
    /// Metres, to the run's centreline.
    pub min_m: f32,
    /// The source's own number and unit ("2 1/4 in").
    #[serde(default)]
    pub as_read: String,
    pub source: Vec<Source>,
}

/// One size of a material. Every length in metres. A number that is absent was not read: never
/// read it as zero.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSize {
    /// Unique across the whole catalog: a connection's pinned `spec` names one.
    pub id: String,
    /// The trade's name for it ("1/2", "14/2", "Cat6").
    pub size: String,
    /// What a picker shows.
    #[serde(default)]
    pub label: String,
    /// The metric nominal size (ISO 6708: DN15 is 1/2 inch), for a pipe, tube, hose or conduit.
    #[serde(default)]
    pub dn: Option<u16>,
    /// Outside diameter; a flat cable's THICKNESS (the way it bends), its width in `width_m`.
    #[serde(default)]
    pub od_m: Option<f32>,
    #[serde(default)]
    pub width_m: Option<f32>,
    /// Inside diameter of a fluid line.
    #[serde(default)]
    pub bore_m: Option<f32>,
    #[serde(default)]
    pub bend_radii: Vec<BendRadius>,
    /// The length it is sold in: a straight stick, or a coil. Couplings fall at stick lengths.
    #[serde(default)]
    pub stick_m: Option<f32>,
    #[serde(default)]
    pub kg_per_m: Option<f32>,
    /// Absolute roughness of the bore, metres (for friction).
    #[serde(default)]
    pub roughness_m: Option<f32>,
    #[serde(default)]
    pub pressure_kpa: Option<f32>,
    #[serde(default)]
    pub temp_max_c: Option<f32>,
    /// American Wire Gauge; a larger number is a thinner wire, 4/0 is -3.
    #[serde(default)]
    pub awg: Option<i32>,
    /// Continuous current, amps. Its presence makes an electrical size offered to the cable check.
    #[serde(default)]
    pub ampacity_a: Option<f32>,
    /// Insulation rating, volts.
    #[serde(default)]
    pub voltage_max: Option<f32>,
    /// Resistance of one conductor, ohms per metre.
    #[serde(default)]
    pub ohm_per_m: Option<f32>,
    #[serde(default)]
    pub shielded: bool,
    /// Litres a minute. Its presence makes a fluid size offered to the checks and pickers.
    #[serde(default)]
    pub flow_max_lpm: Option<f32>,
    /// Megabits a second. Its presence makes a data size offered to the data check.
    #[serde(default)]
    pub bandwidth_mbps: Option<f32>,
    #[serde(default)]
    pub range_m: Option<f32>,
    #[serde(default)]
    pub latency_ms: Option<f32>,
    /// RF emission level 0..1 (a property of the medium, for the detection layer).
    #[serde(default)]
    pub rf_emission: f32,
    #[serde(default)]
    pub grade: Option<Grade>,
    /// Credits a metre, so the auto-pickers choose the cheapest that fits.
    #[serde(default)]
    pub cost_per_m: Option<f32>,
    #[serde(default)]
    pub note: String,
    pub source: Vec<Source>,
}

impl RunSize {
    /// This size's own minimum bend radius for a method, metres, when its row gives one.
    pub fn bend_radius(&self, method: BendMethod) -> Option<f32> {
        self.bend_radii.iter().find(|b| b.method == method).map(|b| b.min_m)
    }
}

/// One material a run can be made of.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunMaterial {
    pub id: String,
    pub name: String,
    pub behaviour: Behaviour,
    pub form: Form,
    /// What it may carry; for a raceway, what its cables may. The FIRST is the content the cable
    /// and data checks file its sizes under.
    pub carries: Vec<Utility>,
    /// For a cable, what conducts; copper tube's is Copper; absent for a wall that conducts
    /// nothing modelled (PEX, PVC).
    #[serde(default)]
    pub conductor: Option<ConductorMaterial>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub look: Look,
    pub sizes: Vec<RunSize>,
    pub source: Vec<Source>,
    #[serde(default)]
    pub note: String,
}

impl RunMaterial {
    /// The content its sizes are checked as: the first it carries.
    pub fn checked_as(&self) -> Option<Utility> {
        self.carries.first().copied()
    }

    /// Is this size offered to the cable and data checks and the connection pickers? Only when the
    /// number its check needs is known: ampacity for electricity, bandwidth for data, flow for a
    /// fluid. A size with no source for that number is left out rather than checked as zero.
    pub fn size_is_checked(&self, size: &RunSize) -> bool {
        match self.checked_as() {
            Some(Utility::Electricity) => size.ampacity_a.is_some(),
            Some(Utility::Data) => size.bandwidth_mbps.is_some(),
            Some(_) => size.flow_max_lpm.is_some(),
            None => false,
        }
    }
}

/// A radio link (WiFi and the like): a data medium with no run at all, nothing laid, bent or
/// supported, so not a material. The data check compares it with the cables.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkRow {
    pub id: String,
    pub label: String,
    pub carries: Utility,
    pub conductor: ConductorMaterial,
    #[serde(default)]
    pub shielded: bool,
    pub grade: Grade,
    pub cost_per_m: f32,
    pub bandwidth_mbps: f32,
    #[serde(default)]
    pub rf_emission: f32,
    pub range_m: f32,
    #[serde(default)]
    pub latency_ms: f32,
    #[serde(default)]
    pub note: String,
    pub source: Vec<Source>,
}

/// The whole catalog: data/piping/pipe_materials.ron.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunMaterials {
    #[serde(default)]
    pub tags: Vec<TagRow>,
    pub kinds: Vec<KindMaterial>,
    pub materials: Vec<RunMaterial>,
    #[serde(default)]
    pub links: Vec<LinkRow>,
}

impl RunMaterials {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn material(&self, id: &str) -> Option<&RunMaterial> {
        self.materials.iter().find(|m| m.id == id)
    }

    /// A size by its id, with the material it is a size of.
    pub fn size(&self, id: &str) -> Option<(&RunMaterial, &RunSize)> {
        self.materials.iter().find_map(|m| m.sizes.iter().find(|s| s.id == id).map(|s| (m, s)))
    }

    /// A size of one material by the trade's name for it ("1/2").
    pub fn size_of(&self, material: &str, size: &str) -> Option<&RunSize> {
        self.material(material)?.sizes.iter().find(|s| s.size == size)
    }

    pub fn link(&self, id: &str) -> Option<&LinkRow> {
        self.links.iter().find(|l| l.id == id)
    }

    /// Is this tag declared in `tags`?
    pub fn tag_declared(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.id == tag)
    }

    /// Every row's sources, named for a problem sentence: each material, size, bend radius and link.
    pub fn sourced_rows(&self) -> Vec<(String, &[Source])> {
        let mut out: Vec<(String, &[Source])> = Vec::new();
        for m in &self.materials {
            out.push((format!("material `{}`", m.id), &m.source));
            for s in &m.sizes {
                out.push((format!("size `{}`", s.id), &s.source));
                for b in &s.bend_radii {
                    out.push((format!("size `{}`'s {:?} bend radius", s.id, b.method), &b.source));
                }
            }
        }
        for l in &self.links {
            out.push((format!("link `{}`", l.id), &l.source));
        }
        out
    }

    /// Everything wrong with the catalog that needs no other file, as sentences (empty = sound):
    /// unique ids (sizes and links share one namespace), declared tags, known kind materials,
    /// numbers in range, the fields a checked size needs, and a source on every row.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut tags = std::collections::HashSet::new();
        for t in &self.tags {
            if !tags.insert(t.id.as_str()) {
                out.push(format!("tag `{}` is declared twice", t.id));
            }
        }
        for k in &self.kinds {
            if self.material(&k.material).is_none() {
                out.push(format!("conduit kind `{}` names unknown material `{}`", k.kind, k.material));
            }
        }
        let mut materials = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for m in &self.materials {
            if !materials.insert(m.id.as_str()) {
                out.push(format!("material `{}` is defined twice", m.id));
            }
            if !(0.0..=1.0).contains(&m.look.metallic) || !(0.0..=1.0).contains(&m.look.roughness) {
                out.push(format!("material `{}`: metallic and roughness must be 0..1", m.id));
            }
            if m.carries.is_empty() {
                out.push(format!("material `{}` carries nothing", m.id));
            }
            if m.sizes.is_empty() {
                out.push(format!("material `{}` has no sizes", m.id));
            }
            for t in &m.tags {
                if !self.tag_declared(t) {
                    out.push(format!("material `{}` carries tag `{t}`, which `tags` does not declare", m.id));
                }
            }
            for s in &m.sizes {
                if !ids.insert(s.id.as_str()) {
                    out.push(format!("size id `{}` is used twice", s.id));
                }
                for (what, v) in [("od_m", s.od_m), ("width_m", s.width_m), ("bore_m", s.bore_m), ("stick_m", s.stick_m)] {
                    if v.is_some_and(|v| !(v > 0.0)) {
                        out.push(format!("size `{}`: {what} must be over 0", s.id));
                    }
                }
                if let (Some(od), Some(bore)) = (s.od_m, s.bore_m) {
                    if bore >= od {
                        out.push(format!("size `{}`: its bore is not inside its outside diameter", s.id));
                    }
                }
                for b in &s.bend_radii {
                    if !(b.min_m > 0.0) {
                        out.push(format!("size `{}`: its {:?} bend radius must be over 0", s.id, b.method));
                    }
                }
                if m.size_is_checked(s) {
                    if s.label.trim().is_empty() {
                        out.push(format!("size `{}` is offered to the checks but has no label", s.id));
                    }
                    if s.grade.is_none() || s.cost_per_m.is_none() {
                        out.push(format!("size `{}` is offered to the checks but has no grade or cost", s.id));
                    }
                }
            }
        }
        for l in &self.links {
            if !ids.insert(l.id.as_str()) {
                out.push(format!("link id `{}` is used twice (sizes and links share ids)", l.id));
            }
        }
        for (row, sources) in self.sourced_rows() {
            out.extend(row_source::shape_problems(&row, sources));
        }
        out
    }

    /// Every source in the catalog that names a finding the findings documents do not define.
    pub fn source_problems(&self, findings: &Findings) -> Vec<String> {
        self.sourced_rows().into_iter().flat_map(|(row, sources)| row_source::problems(&row, sources, findings)).collect()
    }

    /// An empty catalog, for a build whose built-in copy does not parse (the log says so).
    fn empty() -> RunMaterials {
        RunMaterials { tags: Vec::new(), kinds: Vec::new(), materials: Vec::new(), links: Vec::new() }
    }
}

/// The shipped file, the fallback when the data folder's copy is missing or does not parse.
const SHIPPED_RUN_MATERIALS: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));

/// The catalog, loaded once: the data folder's copy first, the shipped copy if that is missing or
/// does not parse (and the log says so, BUG-133).
pub fn run_materials() -> &'static RunMaterials {
    static REG: std::sync::OnceLock<RunMaterials> = std::sync::OnceLock::new();
    REG.get_or_init(load_run_materials)
}

fn load_run_materials() -> RunMaterials {
    let path = crate::storage::data_dir().join("piping").join("pipe_materials.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match RunMaterials::parse(&text) {
            Ok(m) => return m,
            Err(e) => crate::embedded_data::note_builtin_copy(
                "piping/pipe_materials.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => crate::embedded_data::note_builtin_copy(
            "piping/pipe_materials.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    RunMaterials::parse(SHIPPED_RUN_MATERIALS).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/pipe_materials.ron does not parse: {e}");
        RunMaterials::empty()
    })
}

/// One sRGB channel (0-255) to linear light (the IEC 61966-2-1 curve).
pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> RunMaterials {
        RunMaterials::parse(SHIPPED_RUN_MATERIALS).expect("the shipped pipe_materials.ron parses")
    }

    #[test]
    fn shipped_run_materials_registry_parses_and_is_sound() {
        let cat = shipped();
        assert_eq!(cat.problems(), Vec::<String>::new());
        // The three behaviour classes of design 6.1 are all present.
        for b in [Behaviour::Rigid, Behaviour::Bendable, Behaviour::Flexible] {
            assert!(cat.materials.iter().any(|m| m.behaviour == b), "no {b:?} material");
        }
    }

    /// Every source in the catalog names a finding the 2026-11-01 documents define. Seen red with
    /// a size citing R45 (the routing document stops at 44): "size `pex_1` cites R45, which
    /// docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md does not define".
    #[test]
    fn shipped_run_materials_cite_real_findings() {
        let findings = Findings::from_repo(std::path::Path::new(env!("HUMANITY_REPO_ROOT"))).expect("the findings documents");
        assert!(findings.routing.len() >= 44 && findings.doors.len() >= 19, "the documents' findings were read: {findings:?}");
        assert_eq!(shipped().source_problems(&findings), Vec::<String>::new());
    }

    /// THE CDA's 57 mm: CDA Table 4.1's minimum bend radius for 1/2 in annealed copper is 2 1/4 in
    /// (57.15 mm), finding 1, read through the catalog's own lookups.
    #[test]
    fn shipped_catalog_carries_the_cda_radius_for_half_inch_annealed_copper() {
        let cat = shipped();
        let size = cat.size_of("copper_l_annealed", "1/2").expect("1/2 in annealed Type L copper");
        let r = size.bend_radius(BendMethod::Bender).expect("a bender radius");
        assert!((r - 0.05715).abs() < 0.0005, "57 mm (2 1/4 in), got {r} m");
        let row = size.bend_radii.iter().find(|b| b.method == BendMethod::Bender).unwrap();
        assert_eq!(row.source, vec![Source::R(1)], "cited to finding 1");
        assert_eq!(cat.material("copper_l_annealed").unwrap().behaviour, Behaviour::Bendable);
        // Hard copper over 3/4 in has no listed bend (R1): no bender radius on its 1 in row.
        assert!(cat.size_of("copper_l_drawn", "1").unwrap().bend_radius(BendMethod::Bender).is_none());
    }

    /// A misspelt field fails the load rather than vanishing (deny_unknown_fields): the way a typo
    /// in a hand-edited row is caught. Seen passing only with the attribute in place.
    #[test]
    fn a_misspelt_field_does_not_parse() {
        // Each misspelling is checked to have been MADE before its parse is judged: on 2026-11-02 the
        // refutation pass added a source to the row this test matched by its full text, the
        // replacement silently stopped happening, and the test went on asserting about an
        // unchanged file. The anchors are now field names only, not a row's contents.
        let text = SHIPPED_RUN_MATERIALS.replacen("bend_radii:", "bend_radius:", 1);
        assert_ne!(text, SHIPPED_RUN_MATERIALS, "the catalog has a bend_radii field to misspell");
        assert!(RunMaterials::parse(&text).is_err());
        let text = SHIPPED_RUN_MATERIALS.replacen("source: [", "sorce: [", 1);
        assert_ne!(text, SHIPPED_RUN_MATERIALS, "the catalog has a source field to misspell");
        assert!(RunMaterials::parse(&text).is_err(), "a row whose source is misspelt has no source");
    }

    #[test]
    fn srgb_to_linear_matches_the_curve() {
        assert_eq!(srgb_to_linear(0), 0.0);
        assert!((srgb_to_linear(255) - 1.0).abs() < 1e-6);
        // Mid grey 128 is about 0.2158 in linear light.
        assert!((srgb_to_linear(128) - 0.2158).abs() < 1e-3);
    }
}
