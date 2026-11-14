//! The pulley rows: who pulls, the ropes, the sheave bearings, the block rules and the geared
//! hoists, read from `data/pulleys/pulleys.ron` (every number's source is written beside it
//! there). Nothing here holds a number of its own: a new puller, rope or bearing is a row in the
//! file, never a line of code (docs/design/infinite-of-x.md).

use serde::Deserialize;

/// The shipped copy, served only when the file on disk is missing or does not parse.
pub const PULLEYS_RON: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/pulleys/pulleys.ron"));

/// Who or what pulls the rope.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Puller {
    pub id: String,
    pub name_key: String,
    /// The pull it keeps up at the hauling end, newtons.
    pub force_n: f64,
    /// How fast it takes in rope at that pull, metres a second.
    pub speed_m_s: f64,
    /// Rope taken in before it must stop and reset, metres.
    pub stroke_m: f64,
    /// A person or a crew (the advice about geared hoists and crews reads it).
    pub hand: bool,
    pub source: String,
}

/// A rope, by its scaling rules (each in the data file with its source).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Rope {
    pub id: String,
    pub name_key: String,
    /// Working load limit per mm^2 of diameter: WLL = this x d^2 (d in mm), newtons.
    pub wll_n_per_mm2: f64,
    /// Breaking strength over working load limit.
    pub design_factor: f64,
    /// Mass per metre per mm^2 of diameter, kg.
    pub kg_per_m_per_mm2: f64,
    /// The smallest sheave tread diameter as a multiple of the rope diameter.
    pub sheave_ratio: f64,
    /// The block's length as a multiple of the rope diameter where a rule gives one (0 = none).
    pub block_length_ratio: f64,
    /// The sizes the configurator picks from, smallest first, mm.
    pub diameters_mm: Vec<f64>,
    pub source: String,
}

impl Rope {
    /// Working load limit of this rope at `d_mm`, newtons.
    pub fn wll_n(&self, d_mm: f64) -> f64 {
        self.wll_n_per_mm2 * d_mm * d_mm
    }
    /// The pull at which it parts (WLL x design factor), newtons.
    pub fn breaking_n(&self, d_mm: f64) -> f64 {
        self.wll_n(d_mm) * self.design_factor
    }
    /// Mass of one metre at `d_mm`, kg.
    pub fn kg_per_m(&self, d_mm: f64) -> f64 {
        self.kg_per_m_per_mm2 * d_mm * d_mm
    }
    /// The smallest listed diameter whose working limit carries `tension_n`, or None when even
    /// the largest cannot.
    pub fn size_for(&self, tension_n: f64) -> Option<f64> {
        self.diameters_mm.iter().copied().find(|&d| self.wll_n(d) >= tension_n)
    }
}

/// A sheave bearing: the share of the pull that comes out the far side of one sheave when raising.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Bearing {
    pub id: String,
    pub name_key: String,
    pub efficiency: f64,
    pub source: String,
}

/// The rules every block is drawn and sized by, and the search's limits.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BlockRules {
    /// A `data/materials.csv` row: its tensile strength and density.
    pub material: String,
    /// A block's ultimate load over its working load limit.
    pub design_factor: f64,
    pub max_parts_per_stage: u32,
    pub max_stages: u32,
    /// The gap left between a stage's blocks at the top of its travel, metres.
    pub clearance_m: f64,
    /// Rope from the fixed block to where the hauling end is taken in, metres.
    pub lead_m: f64,
    pub source: String,
}

/// A geared hoist the configurator may recommend instead of a tackle.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Hoist {
    pub id: String,
    pub name_key: String,
    pub efficiency: f64,
    pub source: String,
}

/// A block material's two numbers, read from `data/materials.csv`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Ultimate tensile strength, pascals.
    pub tensile_pa: f64,
    /// Density, kg per cubic metre.
    pub density: f64,
}

/// Everything in `data/pulleys/pulleys.ron`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PulleyData {
    pub pullers: Vec<Puller>,
    pub ropes: Vec<Rope>,
    pub bearings: Vec<Bearing>,
    pub block: BlockRules,
    pub hoists: Vec<Hoist>,
}

impl PulleyData {
    /// Parse and check the file's text: every list has a row, every number is in range.
    pub fn parse(text: &str) -> Result<Self, String> {
        let d: PulleyData = ron::from_str(text).map_err(|e| e.to_string())?;
        if d.pullers.is_empty() || d.ropes.is_empty() || d.bearings.is_empty() {
            return Err("pulleys.ron needs at least one puller, rope and bearing".into());
        }
        for p in &d.pullers {
            if !(p.force_n > 0.0 && p.speed_m_s > 0.0 && p.stroke_m > 0.0) {
                return Err(format!("puller {}: force, speed and stroke must be positive", p.id));
            }
        }
        for r in &d.ropes {
            if !(r.wll_n_per_mm2 > 0.0 && r.design_factor >= 1.0 && r.kg_per_m_per_mm2 > 0.0 && r.sheave_ratio > 1.0) {
                return Err(format!("rope {}: strength, design factor, weight and sheave ratio out of range", r.id));
            }
            if r.diameters_mm.is_empty() || r.diameters_mm.windows(2).any(|w| w[1] <= w[0]) {
                return Err(format!("rope {}: diameters must be listed smallest first", r.id));
            }
        }
        for b in &d.bearings {
            if !(b.efficiency > 0.0 && b.efficiency <= 1.0) {
                return Err(format!("bearing {}: efficiency must be in (0, 1]", b.id));
            }
        }
        let k = &d.block;
        if !(k.design_factor >= 1.0 && k.max_parts_per_stage >= 1 && k.max_stages >= 1 && k.clearance_m >= 0.0 && k.lead_m >= 0.0) {
            return Err("block rules out of range".into());
        }
        Ok(d)
    }

    /// Disk first; the shipped copy only when the file is missing or does not parse, and then the
    /// log says so (`embedded_data::note_builtin_copy`).
    pub fn load() -> Self {
        let path = humanity_core::storage::data_dir().join("pulleys").join("pulleys.ron");
        let why = match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => format!("{} does not parse ({e})", path.display()),
            },
            Err(e) => format!("{} could not be read ({e})", path.display()),
        };
        humanity_core::embedded_data::note_builtin_copy("pulleys/pulleys.ron", why);
        Self::parse(PULLEYS_RON).expect("the shipped data/pulleys/pulleys.ron parses")
    }

    pub fn puller(&self, id: &str) -> Option<&Puller> {
        self.pullers.iter().find(|p| p.id == id)
    }
    pub fn rope(&self, id: &str) -> Option<&Rope> {
        self.ropes.iter().find(|r| r.id == id)
    }
    pub fn bearing(&self, id: &str) -> Option<&Bearing> {
        self.bearings.iter().find(|b| b.id == id)
    }
}

/// The block material's tensile strength and density from `data/materials.csv` text (columns
/// `id,name,category,density_kg_m3,tensile_strength_mpa,...`, as its header says).
pub fn material_from_csv(csv: &str, id: &str) -> Option<Material> {
    csv.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split(',').collect::<Vec<_>>())
        .find(|cols| cols.first().is_some_and(|c| c.trim() == id))
        .and_then(|cols| {
            let density = cols.get(3)?.trim().parse::<f64>().ok()?;
            let tensile_mpa = cols.get(4)?.trim().parse::<f64>().ok()?;
            Some(Material { tensile_pa: tensile_mpa * 1.0e6, density })
        })
}

/// The block material named by the rules, from the data folder's `materials.csv`: disk first, the
/// shipped copy only when the file cannot be read, and then the log says so.
pub fn block_material(rules: &BlockRules) -> Option<Material> {
    let path = humanity_core::storage::data_dir().join("materials.csv");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            humanity_core::embedded_data::note_builtin_copy("materials.csv", format!("{} could not be read ({e})", path.display()));
            humanity_core::embedded_data::MATERIALS_CSV.to_string()
        }
    };
    material_from_csv(&text, &rules.material)
}

/// A world the lift happens on, for the Calculator's world choice (docs/design/pulleys.md, "His
/// answers", 2026-11-13): its surface gravity, m/s^2, and where the number comes from. The load's
/// weight is its mass times this gravity, so the same load on the Moon takes about a sixth of the
/// pull it takes on Earth.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    /// The planet file's stem ("moon"), or "ship_rim" for the ship's spin.
    pub id: String,
    /// The planet's own name from its file ("Moon"); empty for a world named by `name_key`.
    pub name: String,
    /// The words' key for a world that has no name in its data (the ship's rim).
    pub name_key: Option<String>,
    pub gravity: f64,
    /// Where the number is written.
    pub source: String,
}

/// The two fields of a planet file (data/planets/<id>.ron) the world choice reads; the rest are
/// the terrain's (`humanity_terrain::planet::PlanetDef`) and are skipped.
#[derive(Deserialize)]
struct PlanetGravity {
    name: String,
    gravity: f64,
}

/// What the world choice reads of the mothership's closed-loop file
/// (data/closed_loop/mothership.ron): the hull's diameter and the habitat's spin period.
#[derive(Deserialize)]
struct Mothership {
    ship: MothershipHull,
    figures: Vec<MothershipFigure>,
}

#[derive(Deserialize)]
struct MothershipHull {
    hull_diameter_m: f64,
}

#[derive(Deserialize)]
struct MothershipFigure {
    id: String,
    #[serde(default)]
    value: Option<f64>,
}

/// The ship's spin gravity at the rim, m/s^2: omega squared times the radius, from the hull's
/// diameter (its radius is the rim) and the habitat's period (`ship.habitat.spin`, "Rotation period
/// for 1 g at a 50 km radius", 449 s): 4 pi^2 x 50,000 / 449^2 = 9.79. None when the file does
/// not say both.
pub fn ship_rim_gravity(mothership_ron: &str) -> Option<f64> {
    let opts = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let m: Mothership = opts.from_str(mothership_ron).map_err(|e| log::warn!("pulleys: the mothership file did not parse for its spin ({e})")).ok()?;
    let period = m.figures.iter().find(|f| f.id == "ship.habitat.spin")?.value?;
    let r = m.ship.hull_diameter_m * 0.5;
    (period > 0.0 && r > 0.0).then(|| 4.0 * std::f64::consts::PI * std::f64::consts::PI * r / (period * period))
}

/// Every world the lift can happen on, from `data_dir`: each planet file's name and gravity
/// (data/planets/*.ron), Earth first and the rest by name, then the ship's spin at its rim when the
/// mothership file says it (`ship_rim_gravity`). A file that does not parse is left out, and the
/// log says so; with none at all, Earth's 9.81 alone, marked as built in.
pub fn worlds_in(data_dir: &std::path::Path) -> Vec<World> {
    let mut out: Vec<World> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(data_dir.join("planets"))
        .map(|it| it.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "ron")).collect())
        .unwrap_or_default();
    files.sort();
    for path in files {
        let id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| ron::from_str::<PlanetGravity>(&t).map_err(|e| e.to_string())) {
            Ok(p) if p.gravity > 0.0 => out.push(World { source: format!("data/planets/{id}.ron"), id, name: p.name, name_key: None, gravity: p.gravity }),
            Ok(_) => {}
            Err(e) => log::warn!("pulleys: {} did not give a world's gravity ({e})", path.display()),
        }
    }
    out.sort_by(|a, b| (a.id != "earth").cmp(&(b.id != "earth")).then(a.name.cmp(&b.name)));
    if out.is_empty() {
        out.push(World { id: "earth".into(), name: "Earth".into(), name_key: None, gravity: 9.81, source: "built in: no planet file could be read".into() });
    }
    let mothership = data_dir.join("closed_loop").join("mothership.ron");
    if let Some(g) = std::fs::read_to_string(&mothership).ok().and_then(|t| ship_rim_gravity(&t)) {
        out.push(World {
            id: "ship_rim".into(),
            name: String::new(),
            name_key: Some("pulley.world_ship_rim".into()),
            gravity: g,
            source: "data/closed_loop/mothership.ron: ship.habitat.spin and hull_diameter_m".into(),
        });
    }
    out
}

/// `worlds_in` the data folder.
pub fn worlds() -> Vec<World> {
    worlds_in(&humanity_core::storage::data_dir())
}
