//! The "to code" rules for runs, as data: `data/piping/run_rules.ron` (2026-11-02, spline routing
//! rung 0, `docs/design/spline-routing.md` section 7.1).
//!
//! Every row is one of the design's rule kinds (bend_radius, fittings_only, support_spacing,
//! support_near, separation, keep_out, vertical_order, no_shared, slope, drain_turn, penetration,
//! pull_limit, fill, allowed_material) with that kind's numbers, what it applies to, the profiles
//! it belongs to (`dwelling_us`, `ship_us`, `spacecraft`: the same cable is strapped every 4 1/2 ft
//! in a manufactured home and every 24 in aboard a small passenger vessel), how binding its source
//! is there (`Standing`), and its `source` (a numbered finding, R or D, or `Ours` with a reason;
//! `humanity_core::row_source`). Every length is in metres and every angle in degrees.
//!
//! NOTHING EVALUATES THESE YET. Rung 8 (`ship::run_checks`) does, on the computed curve, the
//! anchors and the runs in the same cells; rung 9's auto-router reads the vertical order in place
//! of `data/routing_rules.ron`'s lanes. This module is the data model, its loader and the checks
//! that keep the file honest. Plain serde, no `native` gate: the relay will check shared runs.
//!
//! Read from the data folder first and from the copy built into the exe when that is missing or
//! does not parse, which the log says (BUG-133). Read once per run.

use humanity_core::row_source::{self, Findings, Source};
use humanity_core::run_materials::{BendMethod, Behaviour, Form, RunMaterial, RunMaterials, RunSize};
use humanity_core::utilities::Utility;
use serde::Deserialize;

/// How binding a rule's source is where its profile applies, so the editor's words never call a
/// row "code" that is not law there (design 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Standing {
    /// A regulation that binds there (24 CFR 3280 in a manufactured home, 46 CFR aboard a US vessel).
    Law,
    /// A standard the law there incorporates or the work there follows (IEEE 45.8; NASA-STD-8739.4A).
    Standard,
    /// A contract specification, binding only on the projects that adopt it (the DoD's UFGS).
    Specification,
    /// Advice, or a rule for somewhere else used as the closest public rule.
    Guidance,
    /// A maker's data sheet or installation guide.
    Maker,
    /// No public source was read: the project's own number.
    Ours,
}

impl Standing {
    /// May the editor call a finding against this rule "code"? Only where it is law.
    pub fn is_code(self) -> bool {
        self == Standing::Law
    }
}

/// Which runs a rule (or an anchor) is about. Every field is optional; an empty list matches any.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Applies {
    /// Material ids of the catalog.
    pub materials: Vec<String>,
    pub forms: Vec<Form>,
    pub behaviours: Vec<Behaviour>,
    /// What the run carries.
    pub contents: Vec<Utility>,
    /// Material tags, ALL of which the material must carry.
    pub tags: Vec<String>,
    /// Material tags NONE of which it may carry.
    pub without_tags: Vec<String>,
    /// A nominal size (DN) range, inclusive.
    pub dn: Option<(u16, u16)>,
    /// An outside-diameter range, metres, inclusive.
    pub od_m: Option<(f32, f32)>,
    /// A wire gauge range, inclusive (a larger number is a thinner wire).
    pub awg: Option<(i32, i32)>,
    /// The cable's voltage rating range, volts, inclusive.
    pub volts: Option<(f32, f32)>,
}

impl Applies {
    /// Does a run of this material and size, carrying `content`, fall under it? A size-range field
    /// the size has no number for does not match (a cable of unknown diameter is not "under 25 mm").
    pub fn matches(&self, material: &RunMaterial, size: &RunSize, content: Option<Utility>) -> bool {
        (self.materials.is_empty() || self.materials.iter().any(|m| *m == material.id))
            && (self.forms.is_empty() || self.forms.contains(&material.form))
            && (self.behaviours.is_empty() || self.behaviours.contains(&material.behaviour))
            && (self.contents.is_empty()
                || match content {
                    Some(c) => self.contents.contains(&c),
                    None => material.carries.iter().any(|c| self.contents.contains(c)),
                })
            && self.tags.iter().all(|t| material.tags.contains(t))
            && !self.without_tags.iter().any(|t| material.tags.contains(t))
            && self.dn.map_or(true, |(lo, hi)| size.dn.is_some_and(|d| d >= lo && d <= hi))
            && self.od_m.map_or(true, |(lo, hi)| size.od_m.is_some_and(|d| d >= lo && d <= hi))
            && self.awg.map_or(true, |(lo, hi)| size.awg.is_some_and(|g| g >= lo && g <= hi))
            && self.volts.map_or(true, |(lo, hi)| size.voltage_max.is_some_and(|v| v >= lo && v <= hi))
    }

    /// The ids this selector names that the catalog does not declare (materials, tags).
    pub fn unknown_names(&self, catalog: &RunMaterials) -> Vec<String> {
        let mut out = Vec::new();
        for m in &self.materials {
            if catalog.material(m).is_none() {
                out.push(format!("material `{m}`"));
            }
        }
        for t in self.tags.iter().chain(&self.without_tags) {
            if !catalog.tag_declared(t) {
                out.push(format!("tag `{t}`"));
            }
        }
        out
    }
}

/// A minimum bend radius as a rule states it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum BendMin {
    /// A multiple of the run's outside diameter (PEX's 6 times, UTP's 4 times).
    TimesOd(f32),
    /// The size row's own radius for that method (the CDA's table, a maker's sheet).
    SizeRow(BendMethod),
    /// A fixed radius, metres.
    Metres(f32),
}

/// What a support must be near (`support_near`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Near {
    /// A change in direction.
    Bend,
    Valve,
    /// The end of a runout (a branch to a fixture).
    RunoutEnd,
    /// The end of a riser (a vertical run).
    RiserEnd,
    /// A cabinet, box or fitting a cable enters.
    Box,
}

/// What a separation is measured from.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Target {
    /// Other runs.
    Runs(Applies),
    /// Equipment, by the file's `equipment` ids.
    Equipment(Vec<String>),
}

/// How two runs lie for a separation to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Lay {
    /// Wherever they come close.
    #[default]
    Any,
    /// Where they run side by side.
    Parallel,
    /// Where one crosses the other.
    Crossing,
}

/// The volume a keep-out guards around its equipment.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Zone {
    /// The equipment's footprint, from its top up to `height_m` above it, or to the ceiling.
    Above {
        #[serde(default)]
        height_m: Option<f32>,
    },
    /// The working space in front of it.
    InFront { depth_m: f32, width_m: f32 },
}

/// What two contents may not share (`no_shared`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Shared {
    /// A hole through framing.
    Hole,
    /// A hanger or support.
    Support,
    /// A cable bundle.
    Bundle,
}

/// Who a run may not share with.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Partner {
    Runs(Applies),
    /// Its redundant twin: the other half of a redundant system.
    Redundant,
}

/// What a lesser slope needs (`slope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SlopeNeeds {
    /// A full-size cleanout at the upper end.
    CleanoutAtUpperEnd,
}

/// A lesser slope a drain may take, and what it needs to.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReducedSlope {
    pub ratio: f32,
    pub needs: SlopeNeeds,
}

/// A change of direction in a drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Turn {
    HorizontalToVertical,
    VerticalToHorizontal,
    HorizontalToHorizontal,
}

/// A condition on a drain turn; a turn allowed `only` under conditions is allowed when ANY holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TurnCondition {
    /// On a fixture drain of this DN or smaller.
    FixtureDrainUpTo(u16),
    /// On a drain of this DN or larger.
    DnAtLeast(u16),
}

/// One turn a drain fitting may make.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnAllowed {
    pub turn: Turn,
    /// Empty: always. Otherwise only when one of these holds.
    #[serde(default)]
    pub only: Vec<TurnCondition>,
}

/// What a run crosses (`penetration`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Barrier {
    /// A stud or joist it is bored through.
    Stud,
    /// Wood framing generally.
    Wood,
    /// An ordinary interior wall.
    InteriorWall,
    /// Any bulkhead.
    Bulkhead,
    Watertight,
    Oiltight,
    /// A fire-rated boundary (an A or B class bulkhead or deck).
    FireBoundary,
    CollisionBulkhead,
}

/// What a crossing needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PenetrationNeeds {
    /// A steel protection plate.
    Plate,
    /// A hole larger than the run.
    Clearance,
    /// A sleeve with a gasket.
    Sleeve,
    /// The structure's integrity kept (no heat-sensitive metal in a fire boundary).
    Integrity,
    /// Welded on both sides, with a valve.
    WeldedWithValve,
    /// A watertight gland or transit.
    Gland,
    Bushing,
    /// Fire stops along the run.
    FireStop,
}

/// A rule's kind and its numbers (design 7.1), by the design's own word for each.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleKind {
    /// The tightest a run may bend: installed, and where given, preferred, while pulling and
    /// against the coil.
    BendRadius {
        installed: BendMin,
        #[serde(default)]
        preferred: Option<BendMin>,
        #[serde(default)]
        pulling: Option<BendMin>,
        #[serde(default)]
        against_coil: Option<BendMin>,
    },
    /// A rigid material that is never bent.
    FittingsOnly,
    /// The longest unsupported stretch.
    SupportSpacing {
        #[serde(default)]
        horizontal_m: Option<f32>,
        #[serde(default)]
        vertical_m: Option<f32>,
        /// A vertical run is supported at every storey too.
        #[serde(default)]
        every_storey: bool,
        /// The most it may sag between supports.
        #[serde(default)]
        max_sag_m: Option<f32>,
    },
    /// A support near a bend, a valve, a box or an end: `within_m` of it, or the span holding it
    /// at most `span_m`.
    SupportNear {
        near: Near,
        #[serde(default)]
        within_m: Option<f32>,
        #[serde(default)]
        span_m: Option<f32>,
    },
    /// Clearance from other runs or equipment.
    Separation {
        from: Target,
        min_m: f32,
        #[serde(default)]
        lay: Lay,
        /// The clearance to keep whenever possible.
        #[serde(default)]
        preferred_m: Option<f32>,
        /// The clearance allowed where the run is positively clamped.
        #[serde(default)]
        clamped_m: Option<f32>,
        /// Never run alongside.
        #[serde(default)]
        no_parallel: bool,
    },
    /// A volume around equipment the run may not enter.
    KeepOut { equipment: Vec<String>, zone: Zone },
    /// Which contents run above which where routes share, top first, each a group of equals.
    VerticalOrder { top_first: Vec<Vec<Utility>> },
    /// Holes, supports or bundles a run may not share.
    NoShared { what: Shared, with: Partner },
    /// A drain falls toward its outlet: the least rise over run.
    Slope {
        min_ratio: f32,
        #[serde(default)]
        reduced: Option<ReducedSlope>,
    },
    /// Which turns a drain fitting may make.
    DrainTurn { fitting: String, turns: Vec<TurnAllowed> },
    /// How a run crosses a wall, a deck or a stud.
    Penetration {
        through: Vec<Barrier>,
        needs: PenetrationNeeds,
        /// The distance from a stud's face within which it applies.
        #[serde(default)]
        within_m: Option<f32>,
        /// A plate's thickness.
        #[serde(default)]
        thickness_m: Option<f32>,
        /// A hole's clearance around the run.
        #[serde(default)]
        clearance_m: Option<f32>,
        /// No metal melting at or below this in the crossing.
        #[serde(default)]
        min_melt_c: Option<f32>,
        /// A barrier at least this thick may go without (with rounded edges).
        #[serde(default)]
        except_thickness_m: Option<f32>,
        /// Fire stops at least this often vertically and horizontally.
        #[serde(default)]
        every_vertical_m: Option<f32>,
        #[serde(default)]
        every_horizontal_m: Option<f32>,
    },
    /// Bends between pull points, and the pulling force.
    PullLimit {
        #[serde(default)]
        max_bend_deg: Option<f32>,
        #[serde(default)]
        max_tension_n: Option<f32>,
    },
    /// How full a conduit or tray may be, as a fraction of its area.
    Fill {
        max: f32,
        #[serde(default)]
        two_cables: Option<f32>,
        #[serde(default)]
        one_cable: Option<f32>,
    },
    /// Which materials may carry a content.
    AllowedMaterial { contents: Vec<Utility>, materials: Vec<String> },
}

impl RuleKind {
    /// Every kind's word, in the design's order: the check that each has a row reads this.
    pub const IDS: [&'static str; 14] = [
        "bend_radius",
        "fittings_only",
        "support_spacing",
        "support_near",
        "separation",
        "keep_out",
        "vertical_order",
        "no_shared",
        "slope",
        "drain_turn",
        "penetration",
        "pull_limit",
        "fill",
        "allowed_material",
    ];

    /// The design's word for this kind (7.1).
    pub fn id(&self) -> &'static str {
        match self {
            RuleKind::BendRadius { .. } => "bend_radius",
            RuleKind::FittingsOnly => "fittings_only",
            RuleKind::SupportSpacing { .. } => "support_spacing",
            RuleKind::SupportNear { .. } => "support_near",
            RuleKind::Separation { .. } => "separation",
            RuleKind::KeepOut { .. } => "keep_out",
            RuleKind::VerticalOrder { .. } => "vertical_order",
            RuleKind::NoShared { .. } => "no_shared",
            RuleKind::Slope { .. } => "slope",
            RuleKind::DrainTurn { .. } => "drain_turn",
            RuleKind::Penetration { .. } => "penetration",
            RuleKind::PullLimit { .. } => "pull_limit",
            RuleKind::Fill { .. } => "fill",
            RuleKind::AllowedMaterial { .. } => "allowed_material",
        }
    }
}

/// One rule row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRule {
    pub id: String,
    pub kind: RuleKind,
    #[serde(default)]
    pub applies: Applies,
    pub profiles: Vec<String>,
    pub standing: Standing,
    /// The source's own numbers and units.
    #[serde(default)]
    pub as_read: String,
    pub source: Vec<Source>,
    #[serde(default)]
    pub note: String,
}

/// A rule profile: the rules a place follows (a home, a ship's services, a spacecraft).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleProfile {
    pub id: String,
    pub name: String,
    /// What it follows, in words.
    pub draws_on: String,
    pub source: Vec<Source>,
}

/// Equipment a rule can name (a panel, a switchboard, a flue).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentRow {
    pub id: String,
    pub meaning: String,
}

/// The whole file: data/piping/run_rules.ron.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRules {
    pub profiles: Vec<RuleProfile>,
    #[serde(default)]
    pub equipment: Vec<EquipmentRow>,
    pub rules: Vec<RunRule>,
}

impl RunRules {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn profile(&self, id: &str) -> Option<&RuleProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn rule(&self, id: &str) -> Option<&RunRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// The rules of one profile.
    pub fn in_profile<'a>(&'a self, profile: &'a str) -> impl Iterator<Item = &'a RunRule> + 'a {
        self.rules.iter().filter(move |r| r.profiles.iter().any(|p| p == profile))
    }

    /// The rules of a profile that apply to a run of this material and size carrying `content`.
    pub fn applying<'a>(
        &'a self,
        profile: &'a str,
        material: &'a RunMaterial,
        size: &'a RunSize,
        content: Option<Utility>,
    ) -> impl Iterator<Item = &'a RunRule> + 'a {
        self.in_profile(profile).filter(move |r| r.applies.matches(material, size, content))
    }

    /// Every row's sources, named for a problem sentence.
    pub fn sourced_rows(&self) -> Vec<(String, &[Source])> {
        let mut out: Vec<(String, &[Source])> = Vec::new();
        for p in &self.profiles {
            out.push((format!("profile `{}`", p.id), &p.source));
        }
        for r in &self.rules {
            out.push((format!("rule `{}`", r.id), &r.source));
        }
        out
    }

    /// Everything wrong with the file against the catalog and the fittings, as sentences (empty =
    /// sound): unique ids, every rule in at least one known profile, every name it uses declared
    /// (materials, tags, equipment, a drain turn's fitting), numbers in range, every kind of 7.1
    /// with at least one row, every profile with rules, and a source on every row.
    pub fn problems(&self, catalog: &RunMaterials, fittings: &crate::run_fittings::Fittings) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for p in &self.profiles {
            if !seen.insert(p.id.as_str()) {
                out.push(format!("profile `{}` is defined twice", p.id));
            }
            if self.in_profile(&p.id).next().is_none() {
                out.push(format!("profile `{}` has no rules", p.id));
            }
        }
        let equipment_known = |e: &str| self.equipment.iter().any(|row| row.id == e);
        let mut ids = std::collections::HashSet::new();
        for r in &self.rules {
            let row = format!("rule `{}`", r.id);
            if !ids.insert(r.id.as_str()) {
                out.push(format!("{row} is defined twice"));
            }
            if r.profiles.is_empty() {
                out.push(format!("{row} is in no profile"));
            }
            for p in &r.profiles {
                if self.profile(p).is_none() {
                    out.push(format!("{row} names unknown profile `{p}`"));
                }
            }
            for name in r.applies.unknown_names(catalog) {
                out.push(format!("{row} applies to {name}, which the catalog does not declare"));
            }
            let positive = |what: &str, v: f32, out: &mut Vec<String>| {
                if !(v > 0.0) {
                    out.push(format!("{row}: {what} must be over 0"));
                }
            };
            match &r.kind {
                RuleKind::BendRadius { installed, preferred, pulling, against_coil } => {
                    for m in std::iter::once(installed).chain(preferred).chain(pulling).chain(against_coil) {
                        match m {
                            BendMin::TimesOd(x) => positive("a TimesOd multiple", *x, &mut out),
                            BendMin::Metres(x) => positive("a radius", *x, &mut out),
                            BendMin::SizeRow(_) => {}
                        }
                    }
                }
                RuleKind::SupportSpacing { horizontal_m, vertical_m, max_sag_m, .. } => {
                    if horizontal_m.is_none() && vertical_m.is_none() {
                        out.push(format!("{row}: a support spacing needs a horizontal or a vertical number"));
                    }
                    for v in horizontal_m.iter().chain(vertical_m).chain(max_sag_m) {
                        positive("a spacing", *v, &mut out);
                    }
                }
                RuleKind::SupportNear { within_m, span_m, .. } => {
                    if within_m.is_none() && span_m.is_none() {
                        out.push(format!("{row}: support_near needs within_m or span_m"));
                    }
                }
                RuleKind::Separation { from, min_m, .. } => {
                    if *min_m < 0.0 {
                        out.push(format!("{row}: a separation cannot be negative"));
                    }
                    match from {
                        Target::Runs(a) => {
                            for name in a.unknown_names(catalog) {
                                out.push(format!("{row} separates from {name}, which the catalog does not declare"));
                            }
                        }
                        Target::Equipment(list) => {
                            for e in list.iter().filter(|e| !equipment_known(e)) {
                                out.push(format!("{row} names equipment `{e}`, which `equipment` does not declare"));
                            }
                        }
                    }
                }
                RuleKind::KeepOut { equipment, .. } => {
                    if equipment.is_empty() {
                        out.push(format!("{row}: a keep-out needs its equipment"));
                    }
                    for e in equipment.iter().filter(|e| !equipment_known(e)) {
                        out.push(format!("{row} names equipment `{e}`, which `equipment` does not declare"));
                    }
                }
                RuleKind::VerticalOrder { top_first } => {
                    if top_first.len() < 2 {
                        out.push(format!("{row}: an order needs at least two groups"));
                    }
                }
                RuleKind::NoShared { with: Partner::Runs(a), .. } => {
                    for name in a.unknown_names(catalog) {
                        out.push(format!("{row} names {name}, which the catalog does not declare"));
                    }
                }
                RuleKind::Slope { min_ratio, reduced } => {
                    positive("a slope", *min_ratio, &mut out);
                    if let Some(red) = reduced {
                        if !(red.ratio > 0.0 && red.ratio < *min_ratio) {
                            out.push(format!("{row}: a reduced slope must be over 0 and under the slope"));
                        }
                    }
                }
                RuleKind::DrainTurn { fitting, turns } => {
                    if fittings.fitting(fitting).is_none() {
                        out.push(format!("{row} names fitting `{fitting}`, which data/piping/fittings.ron does not have"));
                    }
                    if turns.is_empty() {
                        out.push(format!("{row}: a drain fitting that may turn no way"));
                    }
                }
                RuleKind::PullLimit { max_bend_deg, max_tension_n } => {
                    if max_bend_deg.is_none() && max_tension_n.is_none() {
                        out.push(format!("{row}: a pull limit needs a bend or a tension"));
                    }
                }
                RuleKind::Fill { max, two_cables, one_cable } => {
                    for v in std::iter::once(max).chain(two_cables).chain(one_cable) {
                        if !(*v > 0.0 && *v <= 1.0) {
                            out.push(format!("{row}: a fill is a fraction, over 0 and at most 1"));
                        }
                    }
                }
                RuleKind::AllowedMaterial { contents, materials } => {
                    if contents.is_empty() || materials.is_empty() {
                        out.push(format!("{row}: allowed_material needs contents and materials"));
                    }
                    for m in materials.iter().filter(|m| catalog.material(m).is_none()) {
                        out.push(format!("{row} allows material `{m}`, which the catalog does not have"));
                    }
                }
                RuleKind::FittingsOnly | RuleKind::NoShared { .. } | RuleKind::Penetration { .. } => {}
            }
        }
        for kind in RuleKind::IDS {
            if !self.rules.iter().any(|r| r.kind.id() == kind) {
                out.push(format!("no rule of kind `{kind}`"));
            }
        }
        for (row, sources) in self.sourced_rows() {
            out.extend(row_source::shape_problems(&row, sources));
        }
        out
    }

    /// Every source naming a finding the findings documents do not define.
    pub fn source_problems(&self, findings: &Findings) -> Vec<String> {
        self.sourced_rows().into_iter().flat_map(|(row, sources)| row_source::problems(&row, sources, findings)).collect()
    }
}

/// The shipped file, the fallback when the data folder's copy is missing or does not parse.
const SHIPPED_RUN_RULES: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/run_rules.ron"));

/// The rules, loaded once: the data folder's copy first, the shipped copy if that is missing or
/// does not parse (and the log says so, BUG-133).
pub fn run_rules() -> &'static RunRules {
    static REG: std::sync::OnceLock<RunRules> = std::sync::OnceLock::new();
    REG.get_or_init(load_run_rules)
}

fn load_run_rules() -> RunRules {
    let path = humanity_core::storage::data_dir().join("piping").join("run_rules.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match RunRules::parse(&text) {
            Ok(r) => return r,
            Err(e) => humanity_core::embedded_data::note_builtin_copy(
                "piping/run_rules.ron",
                format_args!("{} does not parse ({e})", path.display()),
            ),
        },
        Err(e) => humanity_core::embedded_data::note_builtin_copy(
            "piping/run_rules.ron",
            format_args!("{} could not be read ({e})", path.display()),
        ),
    }
    RunRules::parse(SHIPPED_RUN_RULES).unwrap_or_else(|e| {
        log::error!("the shipped data/piping/run_rules.ron does not parse: {e}");
        RunRules { profiles: Vec::new(), equipment: Vec::new(), rules: Vec::new() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_fittings::Fittings;

    const SHIPPED_CATALOG: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/pipe_materials.ron"));
    const SHIPPED_FITTINGS: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/piping/fittings.ron"));

    fn shipped() -> (RunRules, RunMaterials, Fittings) {
        (
            RunRules::parse(SHIPPED_RUN_RULES).expect("the shipped run_rules.ron parses"),
            RunMaterials::parse(SHIPPED_CATALOG).expect("the shipped pipe_materials.ron parses"),
            Fittings::parse(SHIPPED_FITTINGS).expect("the shipped fittings.ron parses"),
        )
    }

    /// Seen red on a rule with `source: []`: "rule `support_nm_cable` has no source: name a finding
    /// (R1.., D1..) or give Ours(\"why\")"; and, with the kind's only row removed, "no rule of kind
    /// `fill`".
    #[test]
    fn shipped_run_rules_registry_parses_and_is_sound() {
        let (rules, catalog, fittings) = shipped();
        assert_eq!(rules.problems(&catalog, &fittings), Vec::<String>::new());
        for p in ["dwelling_us", "ship_us", "spacecraft"] {
            assert!(rules.profile(p).is_some(), "the design's profile `{p}`");
        }
    }

    /// 24 CFR 3280.808(d): nonmetallic sheathed cable secured at intervals not exceeding 4 1/2 ft
    /// (1.37 m) in a manufactured home, finding 13, found through the loaders: the dwelling
    /// profile's rules that apply to a size of NM-B cable, not the file's text.
    #[test]
    fn shipped_run_rules_find_the_nm_cable_spacing_in_the_dwelling_profile() {
        let (rules, catalog, _) = shipped();
        let (nm, size) = catalog.size("nm_b_12_2").expect("NM-B 12/2 in the catalog");
        let spacing: Vec<&RunRule> = rules
            .applying("dwelling_us", nm, size, Some(Utility::Electricity))
            .filter(|r| matches!(r.kind, RuleKind::SupportSpacing { .. }))
            .collect();
        assert_eq!(spacing.len(), 1, "one support spacing for NM cable at home: {:?}", spacing.iter().map(|r| &r.id).collect::<Vec<_>>());
        let RuleKind::SupportSpacing { horizontal_m: Some(h), .. } = spacing[0].kind else { panic!("a horizontal spacing") };
        assert!((h - 1.37).abs() < 0.005, "1.37 m (4 1/2 ft), got {h}");
        assert_eq!(spacing[0].source, vec![Source::R(13)]);
        assert_eq!(spacing[0].standing, Standing::Law, "24 CFR 3280 is law in a manufactured home");
        // The same cable aboard a ship follows 46 CFR's 610 mm (R14), not the dwelling's 1.37 m.
        let ship: Vec<f32> = rules
            .applying("ship_us", nm, size, Some(Utility::Electricity))
            .filter_map(|r| match r.kind {
                RuleKind::SupportSpacing { horizontal_m, .. } => horizontal_m,
                _ => None,
            })
            .collect();
        assert_eq!(ship, vec![0.61], "the ship profile's cable support spacing");
    }

    /// Every source names a finding the 2026-11-01 documents define, read from their numbered
    /// paragraphs. Seen red with a rule citing R45: "rule `support_nm_cable` cites R45, which
    /// docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md does not define".
    #[test]
    fn shipped_run_rules_cite_real_findings() {
        let (rules, ..) = shipped();
        let findings = Findings::from_repo(std::path::Path::new(env!("HUMANITY_REPO_ROOT"))).expect("the findings documents");
        assert_eq!(rules.source_problems(&findings), Vec::<String>::new());
    }

    /// The drinking-water rule is the ship profile's alone, and marked as ours (the operator's rule,
    /// an open question in design section 19), never as law.
    #[test]
    fn drinking_water_copper_only_is_the_ships_rule_not_law() {
        let (rules, ..) = shipped();
        let row = rules.rule("allowed_drinking_water_ship").expect("the ship's drinking-water rule");
        assert_eq!(row.profiles, vec!["ship_us".to_string()]);
        assert_eq!(row.standing, Standing::Ours);
        assert!(!row.standing.is_code());
        assert!(matches!(&row.source[..], [Source::Ours(why)] if why.contains("conduits.rs")));
        let RuleKind::AllowedMaterial { materials, .. } = &row.kind else { panic!("allowed_material") };
        assert!(materials.iter().all(|m| m.starts_with("copper")), "copper only: {materials:?}");
    }

    /// A bad row is caught: a rule naming a material the catalog lacks, and a misspelt field.
    #[test]
    fn a_rule_naming_an_unknown_material_or_a_misspelt_field_fails() {
        let (_, catalog, fittings) = shipped();
        let bad = SHIPPED_RUN_RULES.replacen("applies: (materials: [\"nm_b\"]),", "applies: (materials: [\"nm_bb\"]),", 1);
        let rules = RunRules::parse(&bad).expect("still parses");
        let problems = rules.problems(&catalog, &fittings);
        assert!(problems.iter().any(|p| p.contains("`nm_bb`")), "{problems:?}");
        let misspelt = SHIPPED_RUN_RULES.replacen("standing: Law,", "standnig: Law,", 1);
        assert!(RunRules::parse(&misspelt).is_err(), "a misspelt field must not vanish");
    }
}
