//! Data-driven machine layout for the 3D home (First-Playable groundwork, v0.427).
//!
//! Pure data (serde) so it compiles under every feature set, the renderer placement
//! that turns these into primitives + connection pipes lives in `lib.rs::load_world`
//! (native only). Source file: `data/machines/home.ron`.
//!
//! Infinite-of-X: machines and the connections between them are DATA, not code. Add a
//! `catalog` type, an `instance`, or a `connection` to the RON and it appears in the
//! world, no Rust change.

use crate::utilities::{
    check_cable, check_data_link, cheapest_cable_for, cheapest_data_link_for, conduit_type, data_medium,
    CableVerdict, DataVerdict, Port, PortDir, Utility,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Default placement-palette category for an untagged machine type. (v0.527)
fn default_category() -> String {
    "Machines".to_string()
}

/// One readout shown on a machine's info card: an icon (by `kind`), a value, and a
/// status that colors the icon. Placeholder/demo data until the machines are wired to
/// the live simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStat {
    /// "power" | "water" | "storage" | "progress" | "heat" | "fuel" | "nutrient".
    pub kind: String,
    /// Human value, e.g. "120 W", "60%", "idle".
    pub value: String,
    /// "ok" | "warn" | "off" | "low". Colors the icon (green / amber / red-grey / amber).
    pub status: String,
}

/// A machine's role in the live electrical simulation. Spawned as ECS components by
/// `load_world` so the dormant `ElectricalSystem` (and `SolarSystem`) tick against the
/// home's real machines. Optional, so a machine with no electrical role omits it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MachinePower {
    /// Solar panel: output scales with the sun (peak watts at noon, zero at night).
    Solar { peak_watts: f32 },
    /// Steady generator (wind, fuel): constant output while active.
    Generator { watts: f32 },
    /// Power draw. `priority` 1 = critical (shed last), 5 = optional (shed first).
    Consumer { watts: f32, priority: u8 },
    /// Battery bank: buffers surplus / supplies deficit (v0.473). Charges when generation exceeds
    /// consumption, discharges when it falls short, clamped by capacity + the charge/discharge rates.
    Battery { capacity_wh: f32, max_charge_w: f32, max_discharge_w: f32 },
}

/// A machine type: which primitive shape to draw it as, its size, color, display name,
/// and the stat readouts shown on its info card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineDef {
    /// "box" | "cylinder" | "sphere" | "pyramid".
    pub shape: String,
    /// Meters. For box/pyramid: (width, height, depth). For cylinder: (radius, height, _).
    /// For sphere: (radius, _, _).
    pub size: (f32, f32, f32),
    /// Base color, linear 0..1 RGB.
    pub color: (f32, f32, f32),
    /// Display name shown on the floating label (e.g. "Solar panel").
    #[serde(default)]
    pub label: String,
    /// Placement-palette category (e.g. "Power", "Water", "Food", "Production"). Groups the type
    /// in the construction editor's footer palette. Data-driven (infinite-of-X): add a category by
    /// tagging types with it. Defaults to "Machines" so an untagged type still shows. (v0.527)
    #[serde(default = "default_category")]
    pub category: String,
    /// Stat readouts shown on the info card when you are close.
    #[serde(default)]
    pub stats: Vec<MachineStat>,
    /// Electrical role in the live sim (generator / consumer). None = not on the grid.
    #[serde(default)]
    pub power: Option<MachinePower>,
    /// Physical IN/OUT connection PORTS by utility (v0.605, the wiring system -- docs/design/
    /// utility-wiring.md). A teleporter declares an electricity IN; an aeroponic tower water + power
    /// IN. Optional + `#[serde(default)]` so every existing `home.ron` parses unchanged: a def with no
    /// declared ports falls back to `derive_ports()`, which infers electrical ports from `power`.
    #[serde(default)]
    pub ports: Vec<Port>,
    /// Bulk STORAGE this machine provides for a utility (v0.608): a cistern stores water, a silo food, a
    /// tank fuel. Optional + `#[serde(default)]`. Fluid capacity is litres. Drives the live PlumbingSystem
    /// (a cistern's level is a draining number, not a static "33 days" string).
    #[serde(default)]
    pub storage: Vec<MachineStorage>,
    /// RF emission level 0..1 (v0.620): a wireless device (a WiFi router) emits RF while powered, which
    /// harms sensitive crops nearby (run wired instead to keep a grow clean) and is a detection signature.
    /// 0 = a quiet wired/optical device. Spawns an `RfEmitter` ECS component.
    #[serde(default)]
    pub rf_emission: f32,
}

/// Bulk storage a machine provides for one utility (v0.608). `capacity` is litres for a fluid
/// (Water/HotWater/Fuel), or units/kg for a solid (Food/Nutrient).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStorage {
    pub utility: Utility,
    pub capacity: f32,
}

impl MachineDef {
    /// The machine's physical ports: the explicitly-declared `ports` if any, else inferred from the
    /// electrical `power` role (the migration bridge so legacy machines still have real ports for the
    /// buildability conduit check without hand-editing every catalog entry). Electrical inference is
    /// reliable (direction + watts come straight from `MachinePower`); fluid ports must be declared
    /// explicitly because a "water" stat can't tell supply from draw. (v0.605)
    pub fn derive_ports(&self) -> Vec<Port> {
        if !self.ports.is_empty() {
            return self.ports.clone();
        }
        match &self.power {
            Some(MachinePower::Solar { peak_watts }) => vec![Port::elec_out(*peak_watts)],
            Some(MachinePower::Generator { watts }) => vec![Port::elec_out(*watts)],
            Some(MachinePower::Consumer { watts, .. }) => vec![Port::elec_in(*watts)],
            Some(MachinePower::Battery { max_discharge_w, .. }) => vec![Port::elec_bidir(*max_discharge_w)],
            None => Vec::new(),
        }
    }

    /// Total electrical load (watts) this machine DRAWS -- IN ports plus bidirectional terminals (a
    /// battery being charged/discharged). This is the load a feeder cable must carry, the way NEC sizes
    /// a conductor for the load it serves. (v0.605)
    pub fn electrical_load_watts(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| p.utility == Utility::Electricity && matches!(p.dir, PortDir::In | PortDir::Bidirectional))
            .map(|p| p.watts)
            .sum()
    }

    /// Total electrical supply (watts) this machine SOURCES -- the sum of its OUT electrical ports. Used
    /// when a power run feeds something with no declared load (size the cable for the source). (v0.605)
    pub fn electrical_supply_watts(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| p.utility == Utility::Electricity && p.dir == PortDir::Out)
            .map(|p| p.watts)
            .sum()
    }

    /// Whether this machine draws electricity (has an electrical IN / bidirectional port). The water sim
    /// uses this to GATE a powered producer/mover (a pump only moves water while it has power). (v0.608)
    pub fn draws_power(&self) -> bool {
        self.derive_ports()
            .iter()
            .any(|p| p.utility == Utility::Electricity && matches!(p.dir, PortDir::In | PortDir::Bidirectional))
    }

    /// True if this machine participates in the water network: it has a water/hot-water port, or it
    /// stores water. (v0.608)
    pub fn is_water_machine(&self) -> bool {
        let is_w = |u: Utility| matches!(u, Utility::Water | Utility::HotWater);
        self.derive_ports().iter().any(|p| is_w(p.utility))
            || self.storage.iter().any(|s| is_w(s.utility))
    }

    /// Litres/min of water this machine PRODUCES (its water/hot-water OUT ports). (v0.608)
    pub fn water_production_lpm(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| matches!(p.utility, Utility::Water | Utility::HotWater) && p.dir == PortDir::Out)
            .map(|p| p.flow_lpm)
            .sum()
    }

    /// Litres/min of water this machine DRAWS (its water/hot-water IN ports). (v0.608)
    pub fn water_demand_lpm(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| matches!(p.utility, Utility::Water | Utility::HotWater) && p.dir == PortDir::In)
            .map(|p| p.flow_lpm)
            .sum()
    }

    /// Litres of water this machine STORES (a cistern). (v0.608)
    pub fn water_capacity_l(&self) -> f32 {
        self.storage
            .iter()
            .filter(|s| matches!(s.utility, Utility::Water | Utility::HotWater))
            .map(|s| s.capacity)
            .sum()
    }

    /// Megabits/sec of data this machine DEMANDS -- the sum of its Data IN ports. Drives the data-link
    /// sizing check (the chosen medium must carry this over the run). (v0.621)
    pub fn data_demand_mbps(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| p.utility == Utility::Data && p.dir == PortDir::In)
            .map(|p| p.mbps)
            .sum()
    }

    /// Megabits/sec of data this machine SUPPLIES -- the sum of its Data OUT ports (an uplink). (v0.630)
    pub fn data_supply_mbps(&self) -> f32 {
        self.derive_ports()
            .iter()
            .filter(|p| p.utility == Utility::Data && p.dir == PortDir::Out)
            .map(|p| p.mbps)
            .sum()
    }
}

/// One placed machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInstance {
    pub id: String,
    pub machine: String,
    /// A room id. For a HomeStructure (fixed-box) home this is ADVISORY/derived (position no longer
    /// depends on it); for a legacy AABB-room ship layout it binds the instance to a room.
    pub room: String,
    /// Position (v0.538, meaning depends on the home model -- see `MachineHome::placements`):
    /// - HomeStructure box home: ABSOLUTE world (x, y, z) in metres (the box min corner is at the
    ///   world origin; clamped into the box footprint on resolve), y up from the box floor.
    /// - legacy AABB-room ship layout: (x, y, z) RELATIVE to the room center, y up from the floor.
    pub offset: (f32, f32, f32),
}

/// A grid of identical machines, expanded into instances at load time. Lets a dense
/// array (e.g. an indoor garden packed with aeroponic towers) be ONE data line instead
/// of hundreds of hand-typed instances. Infinite-of-X: the array IS the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineArray {
    /// Catalog type to repeat.
    pub machine: String,
    /// Room id to place the grid in (advisory in a HomeStructure box home; see MachineInstance.room).
    pub room: String,
    /// First (row 0, col 0) cell position -- same dual meaning as `MachineInstance.offset`: ABSOLUTE
    /// world x/z in a box home, room-center-relative in a legacy ship layout. `spacing` is a local
    /// step in both. (v0.538)
    pub origin: (f32, f32, f32),
    /// Number of rows (stepped along +z) and columns (stepped along +x).
    pub rows: u32,
    pub cols: u32,
    /// (x_step, z_step) meters between adjacent cells.
    pub spacing: (f32, f32),
    /// Id prefix for the generated instances (e.g. "tower" -> "tower_0", "tower_1", ...).
    pub id_prefix: String,
}

/// A pipe / cable / tube between two machines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConnection {
    pub from: String,
    pub to: String,
    /// "power" | "water" | "nutrient" | "fuel" (colors the tube).
    pub kind: String,
    /// The chosen conduit/cable type id (v0.605, e.g. "cu_awg12"), or None to auto-pick the cheapest
    /// copper that carries the load. `#[serde(default)]` so every existing connection parses unchanged.
    #[serde(default)]
    pub spec: Option<String>,
}

/// One self-sufficiency loop (energy / water / food / nutrients): whether it closes and
/// the honest story. Rendered as the Home-page closure summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeLoop {
    pub name: String,
    pub demand: String,
    pub supply: String,
    /// Does supply + storage meet demand averaged over the worst stretch?
    pub closes: bool,
    /// Is this the binding (weakest) loop, the one that limits overall self-sufficiency?
    #[serde(default)]
    pub weakest: bool,
    pub note: String,
}

/// A conduit junction NODE (v0.581): a draggable point where conduit edges meet / branch. Position is
/// absolute world metres (box home: box min corner at world origin), matching MachineInstance.offset.
/// The node graph is the operator's "edit nodes, software auto-routes the pipe" model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConduitNode {
    pub id: String,
    pub pos: (f32, f32, f32),
    /// Tier in the eventual hierarchy: 0 = main, 1 = sub, 2 = subsub. Stage 1 routes all as 0.
    #[serde(default)]
    pub tier: u8,
    /// Utility-kind hint for colour when an edge doesn't override ("water"|"power"|"gas"|...).
    #[serde(default)]
    pub kind: String,
    /// SERVICE ENTRANCE / GRID TIE (v0.632, grid-hierarchy.md): this node is where the home/zone meets
    /// the EXTERNAL grid (the mothership/fleet main line). Rendered distinctly; the foundation for tying
    /// a home's island into the higher grid tiers. Default false (a plain interior junction).
    #[serde(default)]
    pub grid_tie: bool,
}

/// One endpoint of a conduit edge: a placed MACHINE id or a conduit NODE id. (v0.581)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConduitEnd {
    Machine(String),
    Node(String),
}

/// A routed conduit EDGE between two endpoints (v0.581) -- a graph edge that can pass through junction
/// nodes, routed by the SAME `conduits::route_conduit` a machine-to-machine connection uses today.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConduitEdge {
    pub from: ConduitEnd,
    pub to: ConduitEnd,
    pub kind: String,
}

/// The whole home machine layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineHome {
    /// Machine types, keyed by id. BTreeMap (not HashMap) so `save()` emits the catalog in
    /// stable, sorted key order -- otherwise every save reshuffles all entries (HashMap
    /// iteration is randomized per process), producing a meaningless whole-file git diff and
    /// breaking the home-design parity guarantee that an edit round-trips cleanly. RON loads
    /// maps order-independently, so this is a drop-in change. (v0.522)
    pub catalog: BTreeMap<String, MachineDef>,
    pub instances: Vec<MachineInstance>,
    /// Dense grids expanded into instances at load time. Optional so an older RON parses.
    #[serde(default)]
    pub arrays: Vec<MachineArray>,
    pub connections: Vec<MachineConnection>,
    /// The coupled self-sufficiency loops (energy/water/food/nutrients). Optional so an
    /// older RON without it still parses.
    #[serde(default)]
    pub loops: Vec<HomeLoop>,
    /// Conduit junction NODES (v0.581) -- the node graph the user edits; pipes auto-route through them.
    #[serde(default)]
    pub conduit_nodes: Vec<ConduitNode>,
    /// Conduit EDGES (v0.581) -- node/machine-to-node/machine links, each routed as a real pipe.
    #[serde(default)]
    pub conduit_edges: Vec<ConduitEdge>,
}

/// Pass / warn / fail verdict for one buildability check. Ord follows declaration order
/// (Pass < Warn < Fail), so `worst = worst.max(other)` escalates to the most severe. (v0.605)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

/// One line of the buildability report: a named check, its status, and a human-readable detail.
#[derive(Debug, Clone)]
pub struct BuildabilityCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

/// The buildability report over a home design: "could you actually build + run this on Earth?"
/// Pure + world-free (computed from the placed machines), so it runs in the editor AND an AI can
/// call it before committing a design. (v0.524, home-design Stage 3 -- docs/design/home-design.md)
#[derive(Debug, Clone)]
pub struct BuildabilityReport {
    pub checks: Vec<BuildabilityCheck>,
}

/// A non-punitive USAGE METER for one utility (v0.630, grid S2 -- docs/design/grid-hierarchy.md): the
/// home's daily generation vs demand and how self-sufficient it is. The teaching framing: understand
/// what you actually use + how much of it you make yourself, never a penalty for consuming.
/// `generation`/`demand` are in `unit` (kWh/day for power, L/day for water, Mbps for data).
#[derive(Debug, Clone)]
pub struct UtilityMeter {
    pub utility: String,
    pub generation: f32,
    pub demand: f32,
    /// 0.0..1.0; 1.0 = the home makes at least as much as it uses (self-sufficient).
    pub self_sufficiency: f32,
    pub unit: String,
    pub summary: String,
}

/// Build one meter from a utility's daily generation + demand, with a plain-language, NON-PUNITIVE
/// summary (self-sufficient / surplus to share / amount imported -- never "over budget"). (v0.630)
fn make_utility_meter(utility: &str, generation: f32, demand: f32, unit: &str) -> UtilityMeter {
    let ss = if demand <= 0.0 { 1.0 } else { (generation / demand).min(1.0) };
    let summary = if demand <= 0.0 {
        format!("makes {generation:.1} {unit}; nothing using it yet")
    } else if generation >= demand {
        format!(
            "makes {generation:.1}, uses {demand:.1} {unit} -- fully self-sufficient (+{:.1} to share with the community)",
            generation - demand
        )
    } else {
        format!(
            "makes {generation:.1}, uses {demand:.1} {unit} -- {:.0}% self-sufficient ({:.1} imported from the grid)",
            ss * 100.0,
            demand - generation
        )
    };
    UtilityMeter { utility: utility.to_string(), generation, demand, self_sufficiency: ss, unit: unit.to_string(), summary }
}

/// The geometry of a room the machine placer needs: the floor-plane center (x, z), and the floor
/// and ceiling heights (metres). Plain f32 (no glam) so this module stays renderer-free + testable.
#[derive(Debug, Clone, Copy)]
pub struct RoomGeom {
    pub center_x: f32,
    pub center_z: f32,
    pub floor_y: f32,
    pub ceiling_y: f32,
}

/// A placed machine resolved to its world draw position + appearance, ready for the renderer. The
/// construction editor rebuilds these live on an edit so a move/add/remove shows instantly. (v0.525)
#[derive(Debug, Clone)]
pub struct PlacedMachine {
    pub id: String,
    pub room: String,
    /// World draw position. A sphere is already lifted so it rests on the floor.
    pub pos: (f32, f32, f32),
    /// Y of the machine's top, for the floating label anchor.
    pub top_y: f32,
    /// Room floor + ceiling heights, for the connection-pipe anchor + run sizing.
    pub floor_y: f32,
    pub ceiling_y: f32,
    pub shape: String,
    pub size: (f32, f32, f32),
    pub color: (f32, f32, f32),
    pub label: String,
    pub stats: Vec<MachineStat>,
}

impl BuildabilityReport {
    /// The worst status across all checks (Fail beats Warn beats Pass) -- a one-glance verdict.
    pub fn worst(&self) -> CheckStatus {
        if self.checks.iter().any(|c| c.status == CheckStatus::Fail) {
            CheckStatus::Fail
        } else if self.checks.iter().any(|c| c.status == CheckStatus::Warn) {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        }
    }
}

impl MachineHome {
    /// Load from a RON file. Returns `None` (with a warning) on a missing or invalid
    /// file so the caller can fall back gracefully.
    pub fn load(path: &Path) -> Option<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return None, // absent is fine, distributed builds may omit it
        };
        match ron::from_str::<MachineHome>(&text) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("machines: failed to parse {}: {e}", path.display());
                None
            }
        }
    }

    /// Write the layout back to a RON file -- the construction editor's machine save +
    /// the AI's edit target are the SAME file, so an AI-placed machine is player-editable
    /// and vice versa (the home-design parity principle). A header points at the docs;
    /// the body is anonymous-struct RON, matching the seed's style + always re-loadable.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let config = ron::ser::PrettyConfig::default().struct_names(false);
        let body = ron::ser::to_string_pretty(self, config).map_err(|e| e.to_string())?;
        // Preserve the existing file's LEADING comment block (the authored design header) so a
        // save no longer strips the documentation -- the failure that silently degraded the
        // shipped home.ron when an in-game "Save machines" rewrote it. serde cannot keep comments
        // interspersed with the data, but the top-of-file design rationale is the most valuable and
        // survives this way. Falls back to a pointer-to-docs header if absent or uncommented.
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
            "// HumanityOS home machine layout. Editable in the construction editor or by\n\
             // hand. Real-world energy/water/food model: docs/design/self-sufficiency.md.\n\
             // Design architecture: docs/design/home-design.md.\n\n"
                .to_string()
        });
        std::fs::write(path, format!("{header}{body}")).map_err(|e| e.to_string())
    }

    /// A machine-instance id not already used by ANY placed machine, so the editor can add a
    /// machine without colliding (e.g. "solar_panel_7"). Checks the full id space -- explicit
    /// instances AND every array-expanded cell -- so a generated id can never duplicate an
    /// array cell like "tower_0" (which would silently mis-route connections + stack labels at
    /// load time). (v0.522: was instances-only.)
    pub fn unique_instance_id(&self, base: &str) -> String {
        let all = self.all_instances();
        let used: std::collections::HashSet<&str> = all.iter().map(|i| i.id.as_str()).collect();
        let mut n = 0u32;
        loop {
            let candidate = format!("{base}_{n}");
            if !used.contains(candidate.as_str()) {
                return candidate;
            }
            n += 1;
        }
    }

    /// Remove the explicit instance with this id AND prune every connection that touched it,
    /// so the editor's "Remove" (and an AI edit) never leaves dangling connections pointing at
    /// a machine that no longer exists. Keeps home.ron internally consistent (the
    /// "every connection endpoint is a real instance" invariant the tests assert). (v0.522)
    pub fn remove_instance(&mut self, id: &str) {
        self.instances.retain(|i| i.id != id);
        self.connections.retain(|c| c.from != id && c.to != id);
        // Also prune conduit edges referencing this machine (v0.581), so deleting a machine never
        // leaves a dangling graph edge.
        self.conduit_edges.retain(|e| {
            e.from != ConduitEnd::Machine(id.to_string()) && e.to != ConduitEnd::Machine(id.to_string())
        });
    }

    /// If `id` is an ARRAY-expanded cell (not a direct instance), EXPLODE its whole array into direct
    /// `instances` so each cell becomes individually movable, then return true. The cells keep the EXACT
    /// ids + positions `all_instances()` generated (`{prefix}_{idx}` at origin + step), so any connection
    /// or selection referencing a cell stays valid and nothing visually jumps. A direct instance (or an
    /// unknown id) returns false. This is what makes "drag a grain tray that's part of an array" work --
    /// the editor calls it the moment such a cell is actually dragged. (v0.625)
    pub fn detach_array_member(&mut self, id: &str) -> bool {
        if self.instances.iter().any(|i| i.id == id) {
            return false; // already a direct, movable instance
        }
        let Some(arr_idx) = self.arrays.iter().position(|arr| {
            let count = (arr.rows * arr.cols) as usize;
            (0..count).any(|k| format!("{}_{}", arr.id_prefix, k) == id)
        }) else {
            return false; // not an array cell either -- unknown id
        };
        let arr = self.arrays.remove(arr_idx);
        let mut idx = 0usize;
        for r in 0..arr.rows {
            for c in 0..arr.cols {
                self.instances.push(MachineInstance {
                    id: format!("{}_{}", arr.id_prefix, idx),
                    machine: arr.machine.clone(),
                    room: arr.room.clone(),
                    offset: (
                        arr.origin.0 + c as f32 * arr.spacing.0,
                        arr.origin.1,
                        arr.origin.2 + r as f32 * arr.spacing.1,
                    ),
                });
                idx += 1;
            }
        }
        true
    }

    /// Mint a unique conduit-node id (v0.581), e.g. "node_3".
    pub fn unique_node_id(&self) -> String {
        let mut n = self.conduit_nodes.len();
        loop {
            let id = format!("node_{n}");
            if !self.conduit_nodes.iter().any(|c| c.id == id) {
                return id;
            }
            n += 1;
        }
    }

    /// Add a conduit junction node at `pos`; returns its new id. (v0.581)
    pub fn add_conduit_node(&mut self, pos: (f32, f32, f32), kind: &str) -> String {
        let id = self.unique_node_id();
        self.conduit_nodes.push(ConduitNode { id: id.clone(), pos, tier: 0, kind: kind.to_string(), grid_tie: false });
        id
    }

    /// Move a conduit node; returns true if found. (v0.581)
    pub fn move_conduit_node(&mut self, id: &str, pos: (f32, f32, f32)) -> bool {
        if let Some(n) = self.conduit_nodes.iter_mut().find(|n| n.id == id) {
            n.pos = pos;
            true
        } else {
            false
        }
    }

    /// Remove a conduit node AND prune every edge touching it. (v0.581)
    pub fn remove_conduit_node(&mut self, id: &str) {
        self.conduit_nodes.retain(|n| n.id != id);
        let end = ConduitEnd::Node(id.to_string());
        self.conduit_edges.retain(|e| e.from != end && e.to != end);
    }

    /// Whether a ConduitEnd resolves to a live machine or an existing node. (v0.581)
    fn conduit_end_is_live(&self, end: &ConduitEnd) -> bool {
        match end {
            ConduitEnd::Machine(id) => self.all_instances().into_iter().any(|i| &i.id == id),
            ConduitEnd::Node(id) => self.conduit_nodes.iter().any(|n| &n.id == id),
        }
    }

    /// Add a conduit edge between two endpoints. Refuses a self-edge, a dead endpoint, or an exact
    /// duplicate -- so the editor + an AI can only ever produce valid, routable wiring. (v0.581)
    pub fn add_conduit_edge(&mut self, from: ConduitEnd, to: ConduitEnd, kind: &str) -> bool {
        if from == to || !self.conduit_end_is_live(&from) || !self.conduit_end_is_live(&to) {
            return false;
        }
        if self.conduit_edges.iter().any(|e| e.from == from && e.to == to) {
            return false;
        }
        self.conduit_edges.push(ConduitEdge { from, to, kind: kind.to_string() });
        true
    }

    /// Remove a conduit edge by index; returns true if removed. (v0.581)
    pub fn remove_conduit_edge(&mut self, idx: usize) -> bool {
        if idx < self.conduit_edges.len() {
            self.conduit_edges.remove(idx);
            true
        } else {
            false
        }
    }

    /// Resolve a ConduitEnd to a world anchor (v0.581): a MACHINE uses the SAME low pipe anchor the
    /// renderer uses (placement pos + 0.35 m up); a NODE uses its clamped position. `placements` are
    /// the resolved machine placements; `box_dims` is (width, depth, height) for clamping.
    pub fn conduit_anchor(
        &self,
        end: &ConduitEnd,
        placements: &[(String, (f32, f32, f32), f32)], // (id, pos, floor_y)
        box_dims: (f32, f32, f32),
    ) -> Option<(f32, f32, f32)> {
        match end {
            ConduitEnd::Machine(id) => placements
                .iter()
                .find(|(pid, _, _)| pid == id)
                .map(|(_, pos, floor_y)| (pos.0, floor_y + 0.35, pos.2)),
            ConduitEnd::Node(id) => self.conduit_nodes.iter().find(|n| &n.id == id).map(|n| {
                (
                    n.pos.0.clamp(0.3, box_dims.0 - 0.3),
                    n.pos.1.clamp(0.1, box_dims.2),
                    n.pos.2.clamp(0.3, box_dims.1 - 0.3),
                )
            }),
        }
    }

    /// Drop every machine (explicit instance + array grid) placed in `room_id`, then prune any
    /// connection whose endpoint no longer resolves. Called when a room is deleted in the
    /// construction editor so its machines do not become orphaned -- invisible in-world (the
    /// renderer skips a machine whose room is gone) AND un-removable through the GUI (you can no
    /// longer select the deleted room to reach them). Returns true if anything was removed, so
    /// the caller knows whether to persist. (v0.522)
    ///
    /// NOTE (v0.538): this matches by the stored `.room` STRING id, which is the legacy AABB-room
    /// (ship) model. In a HomeStructure box home `.room` is advisory and machines are positioned
    /// absolutely, so this would not reliably find a machine sitting in a flood-fill region -- but
    /// the box editor (`draw_wall_editor`) deletes machines individually by id via `remove_instance`
    /// and has no room-delete action, so the gap does not manifest there. A geometric
    /// remove-in-AABB is a follow-up if box homes ever gain room deletion.
    pub fn remove_room(&mut self, room_id: &str) -> bool {
        let before = self.instances.len() + self.arrays.len() + self.connections.len();
        self.instances.retain(|i| i.room != room_id);
        self.arrays.retain(|a| a.room != room_id);
        // Prune connections whose endpoints no longer exist among the surviving machines.
        let live: std::collections::HashSet<String> =
            self.all_instances().into_iter().map(|i| i.id).collect();
        self.connections.retain(|c| live.contains(&c.from) && live.contains(&c.to));
        self.instances.len() + self.arrays.len() + self.connections.len() != before
    }

    /// Add a connection (pipe/cable) between two existing machines. Refuses a self-loop, an
    /// empty/unknown endpoint, or an exact (from,to) duplicate, so the editor's connection UI
    /// (and an AI edit) can only ever produce valid, loadable wiring. Returns true if added.
    /// (v0.523)
    pub fn add_connection(&mut self, from: &str, to: &str, kind: &str) -> bool {
        if from == to || from.is_empty() || to.is_empty() {
            return false;
        }
        let live: std::collections::HashSet<String> =
            self.all_instances().into_iter().map(|i| i.id).collect();
        if !live.contains(from) || !live.contains(to) {
            return false;
        }
        if self.connections.iter().any(|c| c.from == from && c.to == to) {
            return false;
        }
        self.connections.push(MachineConnection {
            from: from.to_string(),
            to: to.to_string(),
            kind: kind.to_string(),
            spec: None,
        });
        true
    }

    /// Remove the connection at `idx` (an index into `connections`). Returns true if removed.
    /// (v0.523)
    pub fn remove_connection(&mut self, idx: usize) -> bool {
        if idx < self.connections.len() {
            self.connections.remove(idx);
            true
        } else {
            false
        }
    }

    /// Remove the connection between two machines, in EITHER direction (v0.626). Lets the viewport
    /// "click a pipe -> Remove" gizmo drop a wire by its endpoints without knowing its list index.
    /// Returns true if a connection was removed.
    pub fn remove_connection_between(&mut self, a: &str, b: &str) -> bool {
        let before = self.connections.len();
        self.connections
            .retain(|c| !((c.from == a && c.to == b) || (c.from == b && c.to == a)));
        self.connections.len() != before
    }

    /// Per-utility USAGE METERS for the home (v0.630, grid S2): daily generation vs demand + a self-
    /// sufficiency fraction for power (kWh/day), water (L/day), and data (Mbps). Pure + world-free
    /// (computed from the placed machines' catalog defs), so it runs in the editor + an AI can read it.
    /// Non-punitive: it tells you what you make + use, never penalises consuming. `sun_hours` ~ 4.5.
    pub fn utility_meters(&self, sun_hours: f32) -> Vec<UtilityMeter> {
        let sun = sun_hours.clamp(0.0, 24.0);
        let (mut solar_peak, mut gen_watts, mut consumer_watts) = (0.0f32, 0.0f32, 0.0f32);
        let (mut water_prod, mut water_dem) = (0.0f32, 0.0f32);
        let (mut data_sup, mut data_dem) = (0.0f32, 0.0f32);
        for inst in self.all_instances() {
            if let Some(def) = self.catalog.get(&inst.machine) {
                match &def.power {
                    Some(MachinePower::Solar { peak_watts }) => solar_peak += peak_watts,
                    Some(MachinePower::Generator { watts }) => gen_watts += watts,
                    _ => {}
                }
                consumer_watts += def.electrical_load_watts();
                water_prod += def.water_production_lpm();
                water_dem += def.water_demand_lpm();
                data_sup += def.data_supply_mbps();
                data_dem += def.data_demand_mbps();
            }
        }
        let mut meters = Vec::new();
        // POWER: kWh/day. Solar earns sun_hours of peak; steady generators run 24 h.
        let p_gen = (solar_peak * sun + gen_watts * 24.0) / 1000.0;
        let p_dem = consumer_watts * 24.0 / 1000.0;
        if p_gen > 0.0 || p_dem > 0.0 {
            meters.push(make_utility_meter("power", p_gen, p_dem, "kWh/day"));
        }
        // WATER: L/day (lpm over 1440 min).
        let w_gen = water_prod * 1440.0;
        let w_dem = water_dem * 1440.0;
        if w_gen > 0.0 || w_dem > 0.0 {
            meters.push(make_utility_meter("water", w_gen, w_dem, "L/day"));
        }
        // DATA: Mbps (instantaneous link rate, not a daily total).
        if data_sup > 0.0 || data_dem > 0.0 {
            meters.push(make_utility_meter("data", data_sup, data_dem, "Mbps"));
        }
        meters
    }

    /// A design-time buildability check over the placed machines: is there a power source for the
    /// load, does energy balance over a representative day (with the battery carrying the solar-off
    /// window), and is the wiring intact. Pure + world-free so it runs in the construction editor
    /// AND is callable by an AI before it commits a design. `sun_hours` = representative daily peak-
    /// equivalent sun (the self-sufficiency model uses ~4.5). Real kWh/day, not nameplate -- this is
    /// the home-design real-world-validity guarantee. (v0.524, Stage 3 -- docs/design/home-design.md)
    pub fn buildability_report(&self, sun_hours: f32) -> BuildabilityReport {
        let all = self.all_instances();
        // Sum the electrical roles across every placed machine (via its catalog def's power).
        let mut solar_peak = 0.0f32; // W at full sun
        let mut gen_watts = 0.0f32; // W steady (fuel/wind generators)
        let mut consumer_watts = 0.0f32; // W draw
        let mut battery_wh = 0.0f32; // Wh storage
        for inst in &all {
            if let Some(def) = self.catalog.get(&inst.machine) {
                match &def.power {
                    Some(MachinePower::Solar { peak_watts }) => solar_peak += peak_watts,
                    Some(MachinePower::Generator { watts }) => gen_watts += watts,
                    Some(MachinePower::Consumer { watts, .. }) => consumer_watts += watts,
                    Some(MachinePower::Battery { capacity_wh, .. }) => battery_wh += capacity_wh,
                    None => {}
                }
            }
        }
        let sun = sun_hours.clamp(0.0, 24.0);
        let gen_daily = solar_peak * sun + gen_watts * 24.0; // Wh/day
        let use_daily = consumer_watts * 24.0; // Wh/day
        let mut checks = Vec::new();

        // 1. A power source exists for the load.
        if consumer_watts > 0.0 {
            if solar_peak <= 0.0 && gen_watts <= 0.0 {
                checks.push(BuildabilityCheck {
                    name: "Power source".into(),
                    status: CheckStatus::Fail,
                    detail: format!("{consumer_watts:.0} W of load but no panel or generator"),
                });
            } else {
                checks.push(BuildabilityCheck {
                    name: "Power source".into(),
                    status: CheckStatus::Pass,
                    detail: format!("{solar_peak:.0} W panels + {gen_watts:.0} W generators"),
                });
            }
        }

        // 2. Energy balances over a representative day, battery carries the solar-off window.
        if use_daily > 0.0 {
            if gen_daily + 1.0 < use_daily {
                checks.push(BuildabilityCheck {
                    name: "Energy balance".into(),
                    status: CheckStatus::Fail,
                    detail: format!(
                        "{:.1} kWh/day generated < {:.1} consumed",
                        gen_daily / 1000.0,
                        use_daily / 1000.0
                    ),
                });
            } else {
                // Generation covers the day; can the battery carry the load while solar is off?
                let night_h = (24.0 - sun).max(0.0);
                let night_deficit_w = (consumer_watts - gen_watts).max(0.0);
                let night_need = night_deficit_w * night_h; // Wh the battery must supply overnight
                if battery_wh + 1.0 < night_need {
                    checks.push(BuildabilityCheck {
                        name: "Energy balance".into(),
                        status: CheckStatus::Warn,
                        detail: format!(
                            "{:.1} kWh/day surplus, but battery {:.1} kWh < {:.1} needed overnight",
                            (gen_daily - use_daily) / 1000.0,
                            battery_wh / 1000.0,
                            night_need / 1000.0
                        ),
                    });
                } else {
                    checks.push(BuildabilityCheck {
                        name: "Energy balance".into(),
                        status: CheckStatus::Pass,
                        detail: format!(
                            "{:.1} kWh/day made vs {:.1} used; battery {:.1} kWh carries the night",
                            gen_daily / 1000.0,
                            use_daily / 1000.0,
                            battery_wh / 1000.0
                        ),
                    });
                }
            }
        }

        // 3. Wiring integrity: no connection points at a machine that is not placed (an AI hand-
        //    edit could introduce a dangling reference the editor's add_connection would refuse).
        if !self.connections.is_empty() {
            let live: std::collections::HashSet<&str> = all.iter().map(|i| i.id.as_str()).collect();
            let dangling = self
                .connections
                .iter()
                .filter(|c| !live.contains(c.from.as_str()) || !live.contains(c.to.as_str()))
                .count();
            if dangling > 0 {
                checks.push(BuildabilityCheck {
                    name: "Wiring".into(),
                    status: CheckStatus::Fail,
                    detail: format!("{dangling} connection(s) reference a missing machine"),
                });
            } else {
                checks.push(BuildabilityCheck {
                    name: "Wiring".into(),
                    status: CheckStatus::Pass,
                    detail: format!("{} connection(s), all endpoints valid", self.connections.len()),
                });
            }
        }

        // 4. Conduits (v0.605): every POWER run needs a real copper cable that carries the load it
        //    serves over the run length, within ampacity + <=5% voltage drop. Auto-picks the cheapest
        //    copper that passes (the teaching moment: a lamp run takes thin 14 AWG; an industrial feeder
        //    needs 6 AWG). Run length comes from the machines' world offsets (box-home coords are
        //    absolute world metres). A connection may pin a cable via `spec`; otherwise it's auto-sized.
        let power_runs: Vec<&MachineConnection> =
            self.connections.iter().filter(|c| c.kind == "power").collect();
        if !power_runs.is_empty() {
            const VOLTS: f32 = 120.0; // residential default; per-connection voltage is a later increment
            let by_id: std::collections::HashMap<&str, &MachineInstance> =
                all.iter().map(|i| (i.id.as_str(), i)).collect();
            let dist = |a: (f32, f32, f32), b: (f32, f32, f32)| {
                ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
            };
            let mut worst = CheckStatus::Pass;
            let mut sized = 0usize; // runs we auto-sized or validated OK/marginal
            let mut failing: Vec<String> = Vec::new();
            for c in &power_runs {
                let (Some(from), Some(to)) =
                    (by_id.get(c.from.as_str()), by_id.get(c.to.as_str()))
                else {
                    continue; // a dangling run is already flagged by the Wiring check
                };
                // Load the cable must carry: the destination's draw, else the source's supply.
                let dest_load = self.catalog.get(&to.machine).map(|d| d.electrical_load_watts()).unwrap_or(0.0);
                let load = if dest_load > 0.0 {
                    dest_load
                } else {
                    self.catalog.get(&from.machine).map(|d| d.electrical_supply_watts()).unwrap_or(0.0)
                };
                if load <= 0.0 {
                    continue; // no real electrical load on this run -- nothing to size
                }
                let len = dist(from.offset, to.offset).max(1.0);
                match &c.spec {
                    // An explicitly pinned cable: validate it against the load.
                    Some(id) => match conduit_type(id) {
                        Some(cable) => {
                            let chk = check_cable(cable, load, VOLTS, len);
                            match chk.verdict {
                                CableVerdict::Pass => sized += 1,
                                CableVerdict::Warn => {
                                    sized += 1;
                                    worst = worst.max(CheckStatus::Warn);
                                }
                                CableVerdict::Fail => {
                                    worst = CheckStatus::Fail;
                                    failing.push(format!("{}->{}: {}", c.from, c.to, chk.reason));
                                }
                            }
                        }
                        None => {
                            worst = CheckStatus::Fail;
                            failing.push(format!("{}->{}: unknown cable '{id}'", c.from, c.to));
                        }
                    },
                    // Auto-pick the cheapest copper that carries it.
                    None => match cheapest_cable_for(load, VOLTS, len) {
                        Some(_) => sized += 1,
                        None => {
                            worst = CheckStatus::Fail;
                            failing.push(format!(
                                "{}->{}: no copper carries {load:.0} W over {len:.0} m",
                                c.from, c.to
                            ));
                        }
                    },
                }
            }
            let detail = if failing.is_empty() {
                format!("{sized} power run(s) sized OK (auto-picked cheapest copper that carries the load)")
            } else {
                format!("{}", failing.join("; "))
            };
            // Only emit the check if at least one run had a real load to size (sized + failing > 0).
            if sized > 0 || !failing.is_empty() {
                checks.push(BuildabilityCheck { name: "Conduits".into(), status: worst, detail });
            }
        }

        // 5. Power circuit (v0.606): the operator's "no magic transmission". Every electrical LOAD
        //    must trace through power cabling to a real generation source (panel/generator) -- a load
        //    on a battery-only or unwired circuit can't actually run. Union-find over the power graph.
        if let Some(circuit) = self.power_circuit_check(&all) {
            checks.push(circuit);
        }

        // 6. Data links (v0.621): every DATA run needs a medium (ethernet/fibre/WiFi) that carries the
        //    destination's bandwidth demand over the run length. Auto-pick the cheapest, or validate a
        //    pinned medium. A WIRELESS medium adds an RF caution (it can harm a nearby grow) -- the
        //    telecom teaching moment. Length from the machines' world offsets (box-home coords).
        let data_runs: Vec<&MachineConnection> = self.connections.iter().filter(|c| c.kind == "data").collect();
        if !data_runs.is_empty() {
            let by_id: std::collections::HashMap<&str, &MachineInstance> =
                all.iter().map(|i| (i.id.as_str(), i)).collect();
            let dist = |a: (f32, f32, f32), b: (f32, f32, f32)| {
                ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
            };
            let mut worst = CheckStatus::Pass;
            let mut sized = 0usize;
            let mut notes: Vec<String> = Vec::new();
            for c in &data_runs {
                let (Some(from), Some(to)) = (by_id.get(c.from.as_str()), by_id.get(c.to.as_str())) else {
                    continue; // a dangling run is flagged by the Wiring check
                };
                let demand = self.catalog.get(&to.machine).map(|d| d.data_demand_mbps()).unwrap_or(0.0);
                if demand <= 0.0 {
                    continue; // nothing demands data on this run
                }
                let len = dist(from.offset, to.offset).max(1.0);
                let medium = match &c.spec {
                    Some(id) => data_medium(id),
                    None => cheapest_data_link_for(demand, len),
                };
                match medium {
                    Some(m) => {
                        match check_data_link(m, demand, len).verdict {
                            DataVerdict::Pass => sized += 1,
                            DataVerdict::Warn => {
                                sized += 1;
                                worst = worst.max(CheckStatus::Warn);
                            }
                            DataVerdict::Fail => {
                                worst = CheckStatus::Fail;
                                notes.push(format!("{}->{}: {} can't carry {demand:.0} Mbps over {len:.0} m", c.from, c.to, m.label));
                            }
                        }
                        if m.wireless {
                            // A wireless link emits RF -- caution near a grow (the operator's tradeoff).
                            worst = worst.max(CheckStatus::Warn);
                            notes.push(format!("{}->{}: {} is WIRELESS -- its RF can harm a nearby grow (run wired to stay clean)", c.from, c.to, m.label));
                        }
                    }
                    None => {
                        worst = CheckStatus::Fail;
                        notes.push(match &c.spec {
                            Some(id) => format!("{}->{}: unknown data medium '{id}'", c.from, c.to),
                            None => format!("{}->{}: no medium carries {demand:.0} Mbps over {len:.0} m", c.from, c.to),
                        });
                    }
                }
            }
            if sized > 0 || !notes.is_empty() {
                let detail = if notes.is_empty() {
                    format!("{sized} data run(s) sized OK (auto-picked the cheapest medium that carries the demand)")
                } else {
                    notes.join("; ")
                };
                checks.push(BuildabilityCheck { name: "Data links".into(), status: worst, detail });
            }
        }

        BuildabilityReport { checks }
    }

    /// Union-find over a UTILITY graph (connections + conduit edges of one `kind`, traversing junction
    /// nodes), returning each MEMBER machine id -> its component ROOT id (a stable representative).
    /// Generic over the utility so power + water (+ future air/data) share one tested implementation.
    /// `is_member` decides which machines participate (e.g. has a power role, or has a water port). An
    /// unwired member is its own component. Node endpoints are keyed "node:<id>" so they can't collide
    /// with a machine id. (v0.607/v0.608)
    fn utility_component_roots(
        &self,
        all: &[MachineInstance],
        kind: &str,
        is_member: &dyn Fn(&MachineInstance) -> bool,
    ) -> std::collections::HashMap<String, String> {
        use std::collections::HashMap;
        let mut parent: HashMap<String, String> = HashMap::new();
        fn find(parent: &mut HashMap<String, String>, x: &str) -> String {
            let p = parent.entry(x.to_string()).or_insert_with(|| x.to_string()).clone();
            if p == x {
                return p;
            }
            let root = find(parent, &p);
            parent.insert(x.to_string(), root.clone());
            root
        }
        fn union(parent: &mut HashMap<String, String>, a: &str, b: &str) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                parent.insert(ra, rb);
            }
        }
        // Seed every member machine as its own node (so an unwired member is its own component).
        for inst in all.iter().filter(|i| is_member(i)) {
            find(&mut parent, &inst.id);
        }
        for c in self.connections.iter().filter(|c| c.kind == kind) {
            union(&mut parent, &c.from, &c.to);
        }
        let end_key = |e: &ConduitEnd| match e {
            ConduitEnd::Machine(id) => id.clone(),
            ConduitEnd::Node(id) => format!("node:{id}"),
        };
        for e in self.conduit_edges.iter().filter(|e| e.kind == kind) {
            union(&mut parent, &end_key(&e.from), &end_key(&e.to));
        }
        let mut roots = HashMap::new();
        for inst in all.iter().filter(|i| is_member(i)) {
            let r = find(&mut parent, &inst.id);
            roots.insert(inst.id.clone(), r);
        }
        roots
    }

    /// Each ELECTRICAL machine id -> its component ROOT over the power graph. (v0.607)
    fn power_component_roots(&self, all: &[MachineInstance]) -> std::collections::HashMap<String, String> {
        let is_electrical =
            |inst: &MachineInstance| self.catalog.get(&inst.machine).and_then(|d| d.power.as_ref()).is_some();
        self.utility_component_roots(all, "power", &is_electrical)
    }

    /// Assign 0-based, deterministic ISLAND indices from a roots map (sorted-root order). (v0.607)
    fn islands_from_roots(roots: std::collections::HashMap<String, String>) -> std::collections::HashMap<String, u32> {
        let mut distinct: Vec<&String> = roots.values().collect();
        distinct.sort();
        distinct.dedup();
        let index: std::collections::HashMap<&String, u32> =
            distinct.iter().enumerate().map(|(i, r)| (*r, i as u32)).collect();
        roots.iter().map(|(id, r)| (id.clone(), index[r])).collect()
    }

    /// Map each ELECTRICAL machine id -> a 0-based ISLAND index (its connected power component), for the
    /// runtime `ElectricalSystem` to flow power per island instead of summing globally (sim-realism gap
    /// #2 -- no magic transmission). (v0.607)
    pub fn electrical_islands(&self, all: &[MachineInstance]) -> std::collections::HashMap<String, u32> {
        Self::islands_from_roots(self.power_component_roots(all))
    }

    /// Map each WATER machine id -> a 0-based ISLAND index over the water-pipe graph, so the
    /// `PlumbingSystem` flows water per plumbed circuit (no magic transmission). A "water machine" is one
    /// with a water/hot-water port OR water storage. (v0.608)
    pub fn water_islands(&self, all: &[MachineInstance]) -> std::collections::HashMap<String, u32> {
        let is_water = |inst: &MachineInstance| {
            self.catalog.get(&inst.machine).map(|d| d.is_water_machine()).unwrap_or(false)
        };
        Self::islands_from_roots(self.utility_component_roots(all, "water", &is_water))
    }

    /// The "Power circuit" buildability check (v0.606): build the electrical graph from power
    /// connections + power conduit edges, find connected components (union-find), and verify every
    /// electrical LOAD shares a component with a real generation source (solar/generator). A battery
    /// is STORAGE, not generation -- a load wired only to an uncharged battery (or to nothing) fails,
    /// because no cable carries power to it. Returns None if the home has no electrical machines.
    fn power_circuit_check(&self, all: &[MachineInstance]) -> Option<BuildabilityCheck> {
        // Classify each instance's electrical role from its catalog def.
        #[derive(PartialEq)]
        enum Role {
            Source,  // solar / generator -- real generation
            Load,    // consumer
            Storage, // battery
        }
        let role_of = |inst: &MachineInstance| -> Option<Role> {
            match self.catalog.get(&inst.machine).and_then(|d| d.power.as_ref()) {
                Some(MachinePower::Solar { .. }) | Some(MachinePower::Generator { .. }) => Some(Role::Source),
                Some(MachinePower::Consumer { .. }) => Some(Role::Load),
                Some(MachinePower::Battery { .. }) => Some(Role::Storage),
                None => None,
            }
        };
        let electrical: Vec<(&MachineInstance, Role)> =
            all.iter().filter_map(|i| role_of(i).map(|r| (i, r))).collect();
        if electrical.is_empty() {
            return None; // nothing electrical to check
        }

        // Connected components from the shared power-graph union-find.
        let roots = self.power_component_roots(all);
        // Which components contain real generation (a Source)?
        let mut powered_roots: std::collections::HashSet<&String> = std::collections::HashSet::new();
        for (inst, role) in &electrical {
            if *role == Role::Source {
                if let Some(r) = roots.get(&inst.id) {
                    powered_roots.insert(r);
                }
            }
        }
        // Tally the problems.
        let mut isolated_loads: Vec<String> = Vec::new();
        let mut uncharged_batteries = 0usize;
        for (inst, role) in &electrical {
            let powered = roots.get(&inst.id).map(|r| powered_roots.contains(r)).unwrap_or(false);
            match role {
                Role::Load if !powered => isolated_loads.push(inst.id.clone()),
                Role::Storage if !powered => uncharged_batteries += 1,
                _ => {}
            }
        }

        let (status, detail) = if !isolated_loads.is_empty() {
            let shown: Vec<&str> = isolated_loads.iter().take(4).map(|s| s.as_str()).collect();
            let more = if isolated_loads.len() > 4 { format!(" (+{} more)", isolated_loads.len() - 4) } else { String::new() };
            (
                CheckStatus::Fail,
                format!("{} load(s) not wired to any generator: {}{more}", isolated_loads.len(), shown.join(", ")),
            )
        } else if uncharged_batteries > 0 {
            (
                CheckStatus::Warn,
                format!("all loads powered, but {uncharged_batteries} batter(y/ies) can't reach a generator to charge"),
            )
        } else {
            let loads = electrical.iter().filter(|(_, r)| *r == Role::Load).count();
            (CheckStatus::Pass, format!("{loads} load(s) all trace to a generator through the wiring"))
        };
        Some(BuildabilityCheck { name: "Power circuit".into(), status, detail })
    }

    /// Resolve every placed machine (explicit + array-expanded) to its world draw position +
    /// appearance. Pure + renderer-free: `load_world` turns these into meshes on world entry, and
    /// the construction editor calls this to refresh the machine view LIVE on an edit.
    ///
    /// `box_mode` selects the coordinate model (v0.538):
    /// - **box_mode = true** (a HomeStructure fixed-box home): each instance's `offset.0`/`offset.2`
    ///   is an ABSOLUTE world x/z (the box min corner sits at the world origin), CLAMPED into the box
    ///   footprint `box_dims = (width, depth, height)` so a machine authored with legacy room-relative
    ///   (often negative) coords still lands visibly INSIDE the box; the y base is the box floor (0).
    ///   No instance is skipped on a stale room id -- position no longer depends on the churning
    ///   flood-fill room ids, so a machine survives wall edits and old data still renders.
    /// - **box_mode = false** (a legacy AABB-room ship layout): the offset is RELATIVE to the room
    ///   center, y up from the room floor, and a machine whose room is missing is skipped -- exactly
    ///   as before. `box_dims` is ignored.
    pub fn placements(
        &self,
        rooms: &std::collections::HashMap<String, RoomGeom>,
        box_mode: bool,
        box_dims: (f32, f32, f32),
    ) -> Vec<PlacedMachine> {
        let mut out = Vec::new();
        let (bw, bd, bh) = box_dims;
        for inst in self.all_instances() {
            let Some(def) = self.catalog.get(&inst.machine) else { continue };
            let (x, y, z, floor_y, ceiling_y) = if box_mode {
                // Absolute world x/z clamped into the box; y from the box floor (0).
                let x = inst.offset.0.clamp(0.3, (bw - 0.3).max(0.3));
                let z = inst.offset.2.clamp(0.3, (bd - 0.3).max(0.3));
                (x, inst.offset.1, z, 0.0, bh)
            } else {
                let Some(g) = rooms.get(&inst.room) else { continue };
                (
                    g.center_x + inst.offset.0,
                    g.floor_y + inst.offset.1,
                    g.center_z + inst.offset.2,
                    g.floor_y,
                    g.ceiling_y,
                )
            };
            // A sphere is center-origin; lift it by its radius so it rests on the floor.
            let (pos, top_y) = if def.shape == "sphere" {
                ((x, y + def.size.0, z), y + 2.0 * def.size.0)
            } else {
                ((x, y, z), y + def.size.1)
            };
            out.push(PlacedMachine {
                id: inst.id.clone(),
                room: inst.room.clone(),
                pos,
                top_y,
                floor_y,
                ceiling_y,
                shape: def.shape.clone(),
                size: def.size,
                color: def.color,
                label: if def.label.is_empty() { inst.machine.clone() } else { def.label.clone() },
                stats: def.stats.clone(),
            });
        }
        out
    }

    /// All placed machines: the explicit `instances` plus every `arrays` grid expanded
    /// row-major into individual instances. This is what the renderer should iterate.
    pub fn all_instances(&self) -> Vec<MachineInstance> {
        let mut out = self.instances.clone();
        for arr in &self.arrays {
            let mut idx = 0usize;
            for r in 0..arr.rows {
                for c in 0..arr.cols {
                    out.push(MachineInstance {
                        id: format!("{}_{}", arr.id_prefix, idx),
                        machine: arr.machine.clone(),
                        room: arr.room.clone(),
                        offset: (
                            arr.origin.0 + c as f32 * arr.spacing.0,
                            arr.origin.1,
                            arr.origin.2 + r as f32 * arr.spacing.1,
                        ),
                    });
                    idx += 1;
                }
            }
        }
        out
    }

    /// The placement palette grouped by category: an ordered list of (category, [(id, label)]).
    /// Categories sort alphabetically and items by label, for a stable footer-palette layout.
    /// Data-driven: the categories are whatever the catalog's `category` fields contain. (v0.527)
    pub fn palette_categories(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut by_cat: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (id, def) in &self.catalog {
            let label = if def.label.is_empty() { id.clone() } else { def.label.clone() };
            by_cat.entry(def.category.clone()).or_default().push((id.clone(), label));
        }
        for items in by_cat.values_mut() {
            items.sort_by(|a, b| a.1.cmp(&b.1));
        }
        by_cat.into_iter().collect()
    }

    /// Color (rgba) for a connection kind.
    /// The utility-colour LEGEND (v0.622): one distinct hue per conduit kind, chosen so they read apart
    /// in the build editor's flow markers + pipes. Picked around real conventions (electricity = yellow,
    /// water = blue, hot = red). There are many possible chemicals/utilities, so keep new kinds visually
    /// distinct from these; a fully data-driven utility-colour registry is a future refinement.
    pub fn connection_color(kind: &str) -> [f32; 4] {
        match kind {
            "power" => [0.95, 0.75, 0.15, 1.0],      // amber/yellow (electricity)
            "water" => [0.20, 0.45, 0.85, 1.0],      // blue (potable)
            "hot_water" => [0.90, 0.35, 0.25, 1.0],  // warm red (hot)
            "air" => [0.35, 0.80, 0.90, 1.0],        // cyan (compressed air / ventilation)
            "gas" => [0.70, 0.85, 0.25, 1.0],        // yellow-green (gaseous fuel)
            "fuel" => [0.55, 0.50, 0.18, 1.0],       // olive (liquid fuel / oil)
            "data" => [0.70, 0.35, 0.95, 1.0],       // violet (telecom / internet)
            "nutrient" => [0.55, 0.35, 0.18, 1.0],   // brown (compost / nutrients)
            "waste" => [0.35, 0.40, 0.32, 1.0],      // dark grey-green (sewage / drain)
            "greywater" => [0.55, 0.52, 0.40, 1.0],  // muddy tan (recycled greywater)
            _ => [0.6, 0.6, 0.6, 1.0],               // unknown -> neutral grey
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shipped_home_layout() {
        // Locate data/machines/home.ron relative to the crate root.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron should parse");
        assert!(!home.catalog.is_empty(), "catalog non-empty");
        assert!(!home.instances.is_empty(), "instances non-empty");
        assert!(!home.connections.is_empty(), "connections non-empty");
        // Every array references a known catalog type.
        for arr in &home.arrays {
            assert!(
                home.catalog.contains_key(&arr.machine),
                "array {} references unknown machine type {}",
                arr.id_prefix,
                arr.machine
            );
        }
        // Expanded instances = explicit + every array grid, all referencing known types.
        let all = home.all_instances();
        let expected: usize = home.instances.len()
            + home.arrays.iter().map(|a| (a.rows * a.cols) as usize).sum::<usize>();
        assert_eq!(all.len(), expected, "all_instances() should expand every array grid");
        let mut seen_ids = std::collections::HashSet::new();
        for inst in &all {
            assert!(
                home.catalog.contains_key(&inst.machine),
                "instance {} references unknown machine type {}",
                inst.id,
                inst.machine
            );
            assert!(seen_ids.insert(inst.id.clone()), "duplicate instance id {}", inst.id);
        }
        // Every connection references a defined instance (explicit or array-expanded).
        for c in &home.connections {
            assert!(seen_ids.contains(c.from.as_str()), "connection from unknown {}", c.from);
            assert!(seen_ids.contains(c.to.as_str()), "connection to unknown {}", c.to);
        }
    }

    /// save() round-trips: the seed home.ron, saved + reloaded, preserves catalog +
    /// instances + arrays + connections. This is what makes the construction editor's
    /// machine save (and the AI's edits) safe + loadable.
    #[test]
    fn save_round_trips_the_home_layout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let tmp = std::env::temp_dir().join("humanity_home_roundtrip.ron");
        home.save(&tmp).expect("save");
        let back = MachineHome::load(&tmp).expect("reload saved home");
        assert_eq!(back.catalog.len(), home.catalog.len(), "catalog round-trips");
        assert_eq!(back.instances.len(), home.instances.len(), "instances round-trip");
        assert_eq!(back.arrays.len(), home.arrays.len(), "arrays round-trip");
        assert_eq!(back.connections.len(), home.connections.len(), "connections round-trip");
        // A specific instance keeps its room + machine + offset through the round-trip.
        let first = home.instances.first().expect("at least one instance");
        let found = back.instances.iter().find(|i| i.id == first.id).expect("instance by id");
        assert_eq!(found.room, first.room);
        assert_eq!(found.machine, first.machine);
        assert!((found.offset.0 - first.offset.0).abs() < 1e-6);
        // unique_instance_id avoids existing ids.
        let new_id = home.unique_instance_id("solar_panel");
        assert!(!home.instances.iter().any(|i| i.id == new_id), "id is unused: {new_id}");
        let _ = std::fs::remove_file(&tmp);
    }

    /// The construction editor's "Add machine" flow at the data level: push a new
    /// instance into a room, save, reload -- it persists with the right room + type.
    /// This is the player-can-place-a-machine capability (v0.519 home-design parity).
    #[test]
    fn added_machine_persists_to_room() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let mut home = MachineHome::load(&path).expect("home.ron parses");
        let mtype = home.catalog.keys().next().expect("a catalog type").clone();
        let id = home.unique_instance_id(&mtype);
        home.instances.push(MachineInstance {
            id: id.clone(),
            machine: mtype.clone(),
            room: "garage".to_string(),
            offset: (0.0, 0.0, 0.0),
        });
        let tmp = std::env::temp_dir().join("humanity_home_add.ron");
        home.save(&tmp).expect("save");
        let back = MachineHome::load(&tmp).expect("reload");
        let found = back.instances.iter().find(|i| i.id == id).expect("added machine persisted");
        assert_eq!(found.room, "garage");
        assert_eq!(found.machine, mtype);
        let _ = std::fs::remove_file(&tmp);
    }

    /// A minimal machine def for building synthetic test homes.
    fn test_def(shape: &str) -> MachineDef {
        MachineDef {
            shape: shape.to_string(),
            size: (1.0, 1.0, 1.0),
            color: (0.5, 0.5, 0.5),
            label: String::new(),
            category: "Machines".to_string(),
            stats: Vec::new(),
            power: None,
            ports: Vec::new(),
            storage: Vec::new(),
            rf_emission: 0.0,
        }
    }

    /// v0.522 fix E: unique_instance_id must avoid array-expanded ids, not just explicit
    /// instances -- otherwise "Add solar_panel" could mint "solar_panel_0", colliding with an
    /// array whose id_prefix is "solar_panel" (silently mis-routing connections at load).
    #[test]
    fn unique_id_avoids_array_expanded_cells() {
        let mut catalog = BTreeMap::new();
        catalog.insert("solar_panel".to_string(), test_def("box"));
        let home = MachineHome {
            catalog,
            instances: Vec::new(),
            arrays: vec![MachineArray {
                machine: "solar_panel".to_string(),
                room: "garage".to_string(),
                origin: (0.0, 0.0, 0.0),
                rows: 2,
                cols: 2, // expands to solar_panel_0..solar_panel_3
                spacing: (1.0, 1.0),
                id_prefix: "solar_panel".to_string(),
            }],
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let id = home.unique_instance_id("solar_panel");
        // The four array cells occupy _0.._3, so the next free id must be _4 (not _0).
        let taken: std::collections::HashSet<String> =
            home.all_instances().into_iter().map(|i| i.id).collect();
        assert!(!taken.contains(&id), "generated id {id} collides with an array cell");
        assert_eq!(id, "solar_panel_4");
    }

    /// v0.522 fix B: removing an instance also prunes connections that touched it, so home.ron
    /// never accumulates dangling connections pointing at a deleted machine.
    #[test]
    fn remove_instance_prunes_connections() {
        let mut catalog = BTreeMap::new();
        catalog.insert("pump".to_string(), test_def("box"));
        let inst = |id: &str| MachineInstance {
            id: id.to_string(),
            machine: "pump".to_string(),
            room: "garage".to_string(),
            offset: (0.0, 0.0, 0.0),
        };
        let conn = |from: &str, to: &str| MachineConnection {
            from: from.to_string(),
            to: to.to_string(),
            kind: "water".to_string(),
            spec: None,
        };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: vec![conn("a", "b"), conn("b", "c"), conn("c", "a")],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        home.remove_instance("b");
        assert!(!home.instances.iter().any(|i| i.id == "b"), "instance b removed");
        // a->b and b->c referenced b and must be gone; c->a survives.
        assert_eq!(home.connections.len(), 1, "two connections touching b pruned");
        assert_eq!((&home.connections[0].from, &home.connections[0].to), (&"c".to_string(), &"a".to_string()));
    }

    /// v0.522 fix A: deleting a room drops its machines (instances + arrays) and prunes any now-
    /// dangling connections, so they never become orphaned (invisible + un-removable) dead data.
    #[test]
    fn remove_room_drops_machines_and_prunes_connections() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let inst = |id: &str, room: &str| MachineInstance {
            id: id.to_string(),
            machine: "box".to_string(),
            room: room.to_string(),
            offset: (0.0, 0.0, 0.0),
        };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("g1", "garden"), inst("g2", "garden"), inst("k1", "kitchen")],
            arrays: vec![MachineArray {
                machine: "box".to_string(),
                room: "garden".to_string(),
                origin: (0.0, 0.0, 0.0),
                rows: 1,
                cols: 1,
                spacing: (1.0, 1.0),
                id_prefix: "gtower".to_string(),
            }],
            // g1->k1 spans rooms; deleting garden removes g1 so this connection must go.
            connections: vec![MachineConnection {
                from: "g1".to_string(),
                to: "k1".to_string(),
                kind: "power".to_string(),
                spec: None,
            }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let changed = home.remove_room("garden");
        assert!(changed, "remove_room reports it removed something");
        assert_eq!(home.instances.len(), 1, "only the kitchen instance survives");
        assert_eq!(home.instances[0].id, "k1");
        assert!(home.arrays.is_empty(), "the garden array is dropped");
        assert!(home.connections.is_empty(), "the cross-room connection is pruned");
        // A second delete of a room with nothing in it reports no change.
        assert!(!home.remove_room("garden"), "deleting an empty/absent room is a no-op");
    }

    /// v0.523 Stage 2: add_connection only ever produces valid wiring (no self-loop, no unknown
    /// endpoint, no duplicate), and remove_connection drops by index.
    #[test]
    fn connection_add_validates_and_remove_by_index() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let inst = |id: &str| MachineInstance {
            id: id.to_string(),
            machine: "box".to_string(),
            room: "garage".to_string(),
            offset: (0.0, 0.0, 0.0),
        };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("a"), inst("b")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        assert!(home.add_connection("a", "b", "power"), "valid connection added");
        assert!(!home.add_connection("a", "b", "power"), "exact duplicate refused");
        assert!(!home.add_connection("a", "a", "power"), "self-loop refused");
        assert!(!home.add_connection("a", "ghost", "power"), "unknown endpoint refused");
        assert!(!home.add_connection("", "b", "power"), "empty endpoint refused");
        assert_eq!(home.connections.len(), 1, "only the one valid connection exists");
        assert!(!home.remove_connection(9), "out-of-range index is a no-op");
        assert!(home.remove_connection(0), "in-range index removes");
        assert!(home.connections.is_empty(), "connection removed");
    }

    /// v0.625: detach_array_member explodes an array into direct instances (so a single array cell
    /// becomes movable) keeping the EXACT ids + positions all_instances() generated, and is a no-op
    /// for a direct instance or an unknown id.
    #[test]
    fn detach_array_member_explodes_into_movable_instances() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let mut home = MachineHome {
            catalog,
            instances: vec![MachineInstance { id: "solo".into(), machine: "box".into(), room: "garden".into(), offset: (1.0, 0.0, 2.0) }],
            arrays: vec![MachineArray {
                machine: "box".to_string(),
                room: "garden".to_string(),
                origin: (10.0, 0.0, 20.0),
                rows: 2,
                cols: 3, // 6 cells: tray_0..tray_5
                spacing: (1.5, 2.0),
                id_prefix: "tray".to_string(),
            }],
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        // A direct instance is already movable -> no-op.
        assert!(!home.detach_array_member("solo"), "a direct instance does not detach");
        // An unknown id -> no-op.
        assert!(!home.detach_array_member("ghost"), "an unknown id does not detach");
        // The world position of tray_4 BEFORE detaching (from all_instances).
        let before = home.all_instances().into_iter().find(|i| i.id == "tray_4").unwrap().offset;
        // Detach an array cell -> the whole array explodes into instances.
        assert!(home.detach_array_member("tray_4"), "an array cell detaches");
        assert!(home.arrays.is_empty(), "the array is consumed into instances");
        assert_eq!(home.instances.len(), 1 + 6, "solo + the 6 exploded cells");
        // tray_4 is now a DIRECT instance at the IDENTICAL position (nothing jumps).
        let after = home.instances.iter().find(|i| i.id == "tray_4").expect("tray_4 is now a direct instance");
        assert_eq!(after.offset, before, "the detached cell keeps its exact world position");
        // It is now movable (the editor would write a new offset here).
        // A second detach of the same id is a no-op (it's a direct instance now).
        assert!(!home.detach_array_member("tray_4"), "already-direct cell does not re-detach");
    }

    /// v0.626: remove_connection_between drops a wire by its endpoints in either direction (the
    /// viewport "click a pipe -> Remove" gizmo), and is a no-op for an absent pair.
    #[test]
    fn remove_connection_between_drops_either_direction() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let inst = |id: &str| MachineInstance { id: id.into(), machine: "box".into(), room: "g".into(), offset: (0.0, 0.0, 0.0) };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        assert!(home.add_connection("a", "b", "power"));
        assert!(home.add_connection("b", "c", "water"));
        // Remove a->b by the REVERSED endpoints -> still found.
        assert!(home.remove_connection_between("b", "a"), "either-direction match removes");
        assert_eq!(home.connections.len(), 1, "only b->c remains");
        assert_eq!(home.connections[0].from, "b");
        // An absent pair is a no-op.
        assert!(!home.remove_connection_between("a", "c"), "absent pair is a no-op");
        assert!(home.remove_connection_between("c", "b"), "the forward-or-reverse remaining wire removes");
        assert!(home.connections.is_empty());
    }

    /// v0.632: a conduit node defaults to tier 0 (main) + not a grid tie; both edits survive a RON
    /// round-trip (the trunk hierarchy + service-entrance markers persist with the home).
    #[test]
    fn conduit_node_tier_and_grid_tie_round_trip() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let mut home = MachineHome {
            catalog,
            instances: Vec::new(),
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let id = home.add_conduit_node((1.0, 0.5, 2.0), "power");
        {
            let n = home.conduit_nodes.iter().find(|n| n.id == id).unwrap();
            assert_eq!(n.tier, 0, "a new node defaults to the main tier");
            assert!(!n.grid_tie, "a new node is not a grid tie");
        }
        {
            let n = home.conduit_nodes.iter_mut().find(|n| n.id == id).unwrap();
            n.tier = 2;
            n.grid_tie = true;
        }
        let ron = ron::ser::to_string(&home).expect("serialize");
        let back: MachineHome = ron::from_str(&ron).expect("deserialize");
        let n = back.conduit_nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(n.tier, 2, "tier survives the round-trip");
        assert!(n.grid_tie, "grid_tie survives the round-trip");
    }

    /// A machine def carrying a specific electrical role, for buildability tests.
    fn def_with_power(power: Option<MachinePower>) -> MachineDef {
        MachineDef { power, ..test_def("box") }
    }

    /// v0.524 Stage 3: a load with no panel/generator fails the "Power source" check.
    #[test]
    fn buildability_flags_load_without_a_source() {
        let mut catalog = BTreeMap::new();
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1 })));
        let home = MachineHome {
            catalog,
            instances: vec![MachineInstance { id: "l1".into(), machine: "load".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0) }],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let report = home.buildability_report(4.5);
        assert_eq!(report.worst(), CheckStatus::Fail);
        assert!(report.checks.iter().any(|c| c.name == "Power source" && c.status == CheckStatus::Fail));
    }

    /// v0.630 grid S2: utility_meters reports per-utility daily generation vs demand + a self-sufficiency
    /// fraction, non-punitively. A 1000 W panel + a 100 W load: 4.5 kWh/day made vs 2.4 used -> self-suff.
    #[test]
    fn utility_meters_report_generation_demand_and_self_sufficiency() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1 })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0) };
        let home = MachineHome {
            catalog,
            instances: vec![inst("p1", "panel"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let meters = home.utility_meters(4.5);
        let power = meters.iter().find(|m| m.utility == "power").expect("a power meter exists");
        // gen = 1000 W * 4.5 h / 1000 = 4.5 kWh/day; demand = 100 W * 24 h / 1000 = 2.4 kWh/day.
        assert!((power.generation - 4.5).abs() < 1e-3, "gen {}", power.generation);
        assert!((power.demand - 2.4).abs() < 1e-3, "demand {}", power.demand);
        assert!((power.self_sufficiency - 1.0).abs() < 1e-6, "generation covers demand -> 100% self-sufficient");
        assert!(power.summary.contains("self-sufficient"), "summary frames it non-punitively: {}", power.summary);
        // A pure consumer with no generation reports partial self-sufficiency, never a penalty.
        let m = make_utility_meter("power", 1.0, 4.0, "kWh/day");
        assert!((m.self_sufficiency - 0.25).abs() < 1e-6);
        assert!(m.summary.contains("imported"), "{}", m.summary);
    }

    /// v0.524 Stage 3: panel + battery sized for the night + a modest load passes every check.
    #[test]
    fn buildability_passes_a_balanced_home() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0 })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 2000.0, max_charge_w: 500.0, max_discharge_w: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1 })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "garage".into(), offset: (0.0, 0.0, 0.0) };
        let home = MachineHome {
            catalog,
            instances: vec![inst("p1", "panel"), inst("b1", "batt"), inst("l1", "load")],
            arrays: Vec::new(),
            // Wired panel -> battery -> load so the Power circuit check sees a complete circuit.
            connections: vec![
                MachineConnection { from: "p1".into(), to: "b1".into(), kind: "power".into(), spec: None },
                MachineConnection { from: "b1".into(), to: "l1".into(), kind: "power".into(), spec: None },
            ],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        // 1000W * 4.5h = 4500 Wh/day made vs 100W * 24h = 2400 used; night need = 100W * 19.5h =
        // 1950 Wh <= 2000 Wh battery, so every check passes.
        let report = home.buildability_report(4.5);
        assert_eq!(report.worst(), CheckStatus::Pass, "balanced home passes: {:?}", report.checks);
    }

    /// v0.524 Stage 3: an under-sized battery warns (covers the day, not the night).
    #[test]
    fn buildability_warns_on_undersized_battery() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0 })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 200.0, max_charge_w: 500.0, max_discharge_w: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1 })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "garage".into(), offset: (0.0, 0.0, 0.0) };
        let home = MachineHome {
            catalog,
            instances: vec![inst("p1", "panel"), inst("b1", "batt"), inst("l1", "load")],
            arrays: Vec::new(),
            // Wired panel -> battery -> load so the Power circuit check sees a complete circuit.
            connections: vec![
                MachineConnection { from: "p1".into(), to: "b1".into(), kind: "power".into(), spec: None },
                MachineConnection { from: "b1".into(), to: "l1".into(), kind: "power".into(), spec: None },
            ],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let report = home.buildability_report(4.5);
        assert_eq!(report.worst(), CheckStatus::Warn, "tiny battery warns: {:?}", report.checks);
    }

    /// v0.524 Stage 3: a connection to a missing machine fails the Wiring check.
    #[test]
    fn buildability_flags_a_dangling_connection() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let home = MachineHome {
            catalog,
            instances: vec![MachineInstance { id: "a".into(), machine: "box".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0) }],
            arrays: Vec::new(),
            connections: vec![MachineConnection { from: "a".into(), to: "ghost".into(), kind: "power".into(), spec: None }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let report = home.buildability_report(4.5);
        assert!(report.checks.iter().any(|c| c.name == "Wiring" && c.status == CheckStatus::Fail));
        assert_eq!(report.worst(), CheckStatus::Fail);
    }

    /// v0.524 Stage 3: the shipped seed home produces checks and has intact wiring (it is the
    /// reference design, so its connections must all resolve).
    #[test]
    fn buildability_seed_home_is_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let report = home.buildability_report(4.5);
        assert!(!report.checks.is_empty(), "seed home produces checks");
        assert!(
            !report.checks.iter().any(|c| c.name == "Wiring" && c.status == CheckStatus::Fail),
            "seed wiring must be intact: {:?}",
            report.checks
        );
    }

    fn pos_test_home() -> MachineHome {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let mut sphere_def = test_def("sphere");
        sphere_def.size = (0.5, 0.0, 0.0); // radius 0.5
        catalog.insert("ball".to_string(), sphere_def);
        MachineHome {
            catalog,
            instances: vec![
                MachineInstance { id: "b1".into(), machine: "box".into(), room: "garage".into(), offset: (1.0, 0.0, 2.0) },
                MachineInstance { id: "s1".into(), machine: "ball".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0) },
                MachineInstance { id: "ghost".into(), machine: "box".into(), room: "nowhere".into(), offset: (0.0, 0.0, 0.0) },
            ],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        }
    }

    /// v0.525/v0.538: in SHIP mode (box_mode=false) placements() resolves room center + offset,
    /// floor-relative y, lifts spheres, and SKIPS a machine whose room has no geometry.
    #[test]
    fn placements_ship_mode_is_room_relative_and_skips() {
        let home = pos_test_home();
        let mut rooms = std::collections::HashMap::new();
        rooms.insert("garage".to_string(), RoomGeom { center_x: 10.0, center_z: 20.0, floor_y: 5.0, ceiling_y: 8.0 });
        let placed = home.placements(&rooms, false, (0.0, 0.0, 0.0));
        assert_eq!(placed.len(), 2, "the machine in an unknown room is skipped in ship mode");
        let b = placed.iter().find(|p| p.id == "b1").unwrap();
        assert_eq!(b.pos, (11.0, 5.0, 22.0), "box at center+offset, floor-relative");
        let s = placed.iter().find(|p| p.id == "s1").unwrap();
        assert_eq!(s.pos, (10.0, 5.5, 20.0), "sphere lifted by its radius to rest on the floor");
        assert_eq!(s.floor_y, 5.0);
    }

    #[test]
    fn conduit_graph_nodes_edges_and_pruning() {
        let mut home = pos_test_home();
        let ids: Vec<String> = home.all_instances().into_iter().map(|i| i.id).collect();
        assert!(ids.len() >= 2);
        let nid = home.add_conduit_node((10.0, 1.0, 10.0), "water");
        assert_eq!(home.conduit_nodes.len(), 1);
        // machine -> node, node -> machine
        assert!(home.add_conduit_edge(ConduitEnd::Machine(ids[0].clone()), ConduitEnd::Node(nid.clone()), "water"));
        assert!(home.add_conduit_edge(ConduitEnd::Node(nid.clone()), ConduitEnd::Machine(ids[1].clone()), "water"));
        assert_eq!(home.conduit_edges.len(), 2);
        // refuse self / dead-endpoint / duplicate
        assert!(!home.add_conduit_edge(ConduitEnd::Node(nid.clone()), ConduitEnd::Node(nid.clone()), "water"));
        assert!(!home.add_conduit_edge(ConduitEnd::Node("nope".into()), ConduitEnd::Machine(ids[0].clone()), "water"));
        assert!(!home.add_conduit_edge(ConduitEnd::Machine(ids[0].clone()), ConduitEnd::Node(nid.clone()), "water"));
        assert!(home.move_conduit_node(&nid, (12.0, 1.5, 12.0)));
        assert_eq!(home.conduit_nodes[0].pos, (12.0, 1.5, 12.0));
        // removing the node prunes both edges; removing a machine prunes its edge too
        home.remove_conduit_node(&nid);
        assert_eq!(home.conduit_nodes.len(), 0);
        assert!(home.conduit_edges.is_empty(), "edges touching the node are pruned");
        let n2 = home.add_conduit_node((5.0, 1.0, 5.0), "power");
        assert!(home.add_conduit_edge(ConduitEnd::Machine(ids[0].clone()), ConduitEnd::Node(n2), "power"));
        home.remove_instance(&ids[0]);
        assert!(home.conduit_edges.is_empty(), "removing a machine prunes its conduit edges");
    }

    /// v0.538: in BOX mode (box_mode=true) offset is ABSOLUTE world x/z, NOTHING is skipped on a
    /// stale room id, floor/ceiling come from the box, and a sphere still lifts off floor 0.
    #[test]
    fn placements_box_mode_is_absolute_and_never_skips() {
        let home = pos_test_home();
        let rooms = std::collections::HashMap::new(); // empty -- box mode must not depend on it
        let placed = home.placements(&rooms, true, (55.0, 89.0, 3.0));
        assert_eq!(placed.len(), 3, "box mode skips nothing -- all three render");
        let b = placed.iter().find(|p| p.id == "b1").unwrap();
        assert_eq!(b.pos, (1.0, 0.0, 2.0), "box at its absolute offset, y on the box floor");
        assert_eq!(b.floor_y, 0.0);
        assert_eq!(b.ceiling_y, 3.0, "ceiling from the box height");
        let s = placed.iter().find(|p| p.id == "s1").unwrap();
        // offset (0,0,0) clamps to (0.3, _, 0.3); sphere lifts by radius 0.5.
        assert!((s.pos.0 - 0.3).abs() < 1e-5 && (s.pos.2 - 0.3).abs() < 1e-5);
        assert!((s.pos.1 - 0.5).abs() < 1e-5, "sphere rests on floor 0 lifted by its radius");
    }

    /// v0.538: a negative legacy offset (a real shipped value) still RESOLVES (visible) in box mode,
    /// clamped into the box footprint rather than dropped.
    #[test]
    fn placements_box_mode_clamps_negative_offsets_into_the_box() {
        let mut home = pos_test_home();
        home.instances = vec![MachineInstance { id: "solar".into(), machine: "box".into(), room: "garage".into(), offset: (-7.0, 0.0, -22.0) }];
        let placed = home.placements(&std::collections::HashMap::new(), true, (55.0, 89.0, 3.0));
        assert_eq!(placed.len(), 1, "a negative-offset machine is visible, not skipped");
        assert!((placed[0].pos.0 - 0.3).abs() < 1e-5, "negative x clamps to the near edge, inside the box");
        assert!((placed[0].pos.2 - 0.3).abs() < 1e-5, "negative z clamps to the near edge, inside the box");
    }

    /// v0.538: the shipped home.ron renders EVERY machine in box mode (the direct regression test for
    /// the room-id-mismatch breakage -- placed count == all_instances count, nothing skipped).
    #[test]
    fn shipped_home_renders_all_machines_in_box_mode() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let placed = home.placements(&std::collections::HashMap::new(), true, (55.0, 89.0, 3.0));
        assert_eq!(placed.len(), home.all_instances().len(), "box mode skips nothing -- the seed home renders fully");
    }

    /// v0.527: palette_categories groups the catalog by `category`, sorted, with every machine in
    /// exactly one category -- the data the footer placement palette renders.
    #[test]
    fn palette_groups_catalog_by_category() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let cats = home.palette_categories();
        let total: usize = cats.iter().map(|(_, items)| items.len()).sum();
        assert_eq!(total, home.catalog.len(), "every machine appears in exactly one category");
        let power = cats.iter().find(|(c, _)| c == "Power").expect("a Power category");
        assert!(power.1.iter().any(|(id, _)| id == "solar_panel"), "solar panel is under Power");
        let names: Vec<String> = cats.iter().map(|(c, _)| c.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "categories are sorted alphabetically");
    }

    /// v0.522 fix C: save() is deterministic -- the same home saved twice produces byte-identical
    /// output. Before the BTreeMap switch the HashMap catalog reshuffled on every save, producing
    /// a meaningless whole-file diff and breaking the home-design parity (clean round-trip) rule.
    #[test]
    fn save_is_deterministic() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let a = std::env::temp_dir().join("humanity_home_det_a.ron");
        let b = std::env::temp_dir().join("humanity_home_det_b.ron");
        home.save(&a).expect("save a");
        home.save(&b).expect("save b");
        let ta = std::fs::read_to_string(&a).unwrap();
        let tb = std::fs::read_to_string(&b).unwrap();
        assert_eq!(ta, tb, "two saves of the same home must be byte-identical");
        // And a reload then re-save is identical too (round-trip stability).
        let reloaded = MachineHome::load(&a).expect("reload");
        let c = std::env::temp_dir().join("humanity_home_det_c.ron");
        reloaded.save(&c).expect("save c");
        assert_eq!(ta, std::fs::read_to_string(&c).unwrap(), "reload+save round-trips byte-identically");
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
        let _ = std::fs::remove_file(&c);
    }

    /// v0.525 fix: save() preserves the existing file's leading comment block (the authored design
    /// header), so an in-game save no longer strips the documentation (the regression that degraded
    /// the shipped home.ron).
    #[test]
    fn save_preserves_the_leading_design_header() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let tmp = std::env::temp_dir().join("humanity_home_header.ron");
        home.save(&tmp).expect("first save");
        // Prepend a sentinel design note to the leading comment block, then reload + re-save.
        let with_note = format!("// SENTINEL_KEEP_ME design note\n{}", std::fs::read_to_string(&tmp).unwrap());
        std::fs::write(&tmp, with_note).unwrap();
        let reloaded = MachineHome::load(&tmp).expect("reload with the sentinel note");
        reloaded.save(&tmp).expect("re-save");
        let after = std::fs::read_to_string(&tmp).unwrap();
        assert!(after.contains("SENTINEL_KEEP_ME"), "the leading design header survives a save");
        // And the data still round-trips intact.
        let back = MachineHome::load(&tmp).expect("reload after re-save");
        assert_eq!(back.catalog.len(), home.catalog.len());
        let _ = std::fs::remove_file(&tmp);
    }

    // -- v0.605 wiring Stage 2: ports + the Conduits buildability check --------------------------

    /// A two-machine home wired source -> load, machines `gap` metres apart on a single power run.
    fn wired_pair(src: MachineDef, load: MachineDef, gap: f32, spec: Option<&str>) -> MachineHome {
        let mut catalog = BTreeMap::new();
        catalog.insert("src".to_string(), src);
        catalog.insert("load".to_string(), load);
        MachineHome {
            catalog,
            instances: vec![
                MachineInstance { id: "s1".into(), machine: "src".into(), room: "g".into(), offset: (0.0, 0.0, 0.0) },
                MachineInstance { id: "l1".into(), machine: "load".into(), room: "g".into(), offset: (gap, 0.0, 0.0) },
            ],
            arrays: Vec::new(),
            connections: vec![MachineConnection {
                from: "s1".into(),
                to: "l1".into(),
                kind: "power".into(),
                spec: spec.map(|s| s.to_string()),
            }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        }
    }

    /// derive_ports infers an electrical port from the `power` role, explicit ports override it, and
    /// the load helper sums IN + bidirectional electrical ports.
    #[test]
    fn derive_ports_infers_from_power_and_explicit_wins() {
        let consumer = def_with_power(Some(MachinePower::Consumer { watts: 200.0, priority: 1 }));
        assert_eq!(consumer.derive_ports().len(), 1, "a consumer infers one IN port");
        assert_eq!(consumer.electrical_load_watts(), 200.0);
        let panel = def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0 }));
        assert_eq!(panel.electrical_supply_watts(), 1000.0, "a panel supplies, draws nothing");
        assert_eq!(panel.electrical_load_watts(), 0.0);
        // Explicit ports win over the power-role inference.
        let mut explicit = test_def("box");
        explicit.ports =
            vec![crate::utilities::Port::fluid_in(crate::utilities::Utility::Water, 14.0), crate::utilities::Port::elec_in(50.0)];
        assert_eq!(explicit.derive_ports().len(), 2, "explicit ports are used verbatim");
        assert_eq!(explicit.electrical_load_watts(), 50.0, "only the electrical IN port counts as load");
    }

    /// A modest load over a short run auto-sizes to the cheapest copper and PASSES the Conduits check.
    #[test]
    fn buildability_conduits_autosize_passes() {
        let home = wired_pair(
            def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 120.0, priority: 1 })),
            2.0,
            None,
        );
        let report = home.buildability_report(4.5);
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Pass, "120 W over 2 m auto-sizes: {}", conduit.detail);
    }

    /// A pinned, undersized cable feeding a heavy load FAILS -- "not all cables can handle all power".
    #[test]
    fn buildability_conduits_undersized_pinned_cable_fails() {
        let home = wired_pair(
            def_with_power(Some(MachinePower::Generator { watts: 5000.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 3000.0, priority: 1 })),
            1.0,
            Some("cu_awg14"), // 15 A cable; 3000 W @ 120 V = 25 A -> over ampacity
        );
        let report = home.buildability_report(4.5);
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Fail, "25 A on a 15 A cable fails: {}", conduit.detail);
    }

    /// A pinned cable id that isn't in the registry FAILS the check (an AI/hand edit can't reference a
    /// nonexistent cable and have it silently pass).
    #[test]
    fn buildability_conduits_unknown_cable_id_fails() {
        let home = wired_pair(
            def_with_power(Some(MachinePower::Generator { watts: 500.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 200.0, priority: 1 })),
            1.0,
            Some("unobtainium_42"),
        );
        let report = home.buildability_report(4.5);
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Fail, "unknown cable id fails: {}", conduit.detail);
    }

    /// v0.621 telecom Stage 2: a DATA run sized to a wired medium PASSES; the same run on WiFi WARNS
    /// (its RF can harm a nearby grow). Builds the uplink -> server pair the Data-links check validates.
    #[test]
    fn buildability_data_links_wired_passes_wifi_warns_on_rf() {
        let mut uplink = test_def("box");
        uplink.ports = vec![crate::utilities::Port::data_out(1000.0)];
        let mut server = test_def("box");
        server.ports = vec![crate::utilities::Port::data_in(100.0)];
        let mut catalog = BTreeMap::new();
        catalog.insert("uplink".to_string(), uplink);
        catalog.insert("server".to_string(), server);
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0) };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("u", "uplink"), inst("s", "server")],
            arrays: Vec::new(),
            connections: vec![MachineConnection { from: "u".into(), to: "s".into(), kind: "data".into(), spec: Some("eth_cat6".into()) }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let wired = home.buildability_report(4.5);
        let d = wired.checks.iter().find(|c| c.name == "Data links").expect("a Data links check");
        assert_eq!(d.status, CheckStatus::Pass, "wired Cat6 carries 100 Mbps: {}", d.detail);

        // Swap to WiFi: it still carries the bandwidth, but the wireless RF warning fires.
        home.connections[0].spec = Some("wifi_6".to_string());
        let wifi = home.buildability_report(4.5);
        let d2 = wifi.checks.iter().find(|c| c.name == "Data links").unwrap();
        assert_eq!(d2.status, CheckStatus::Warn, "wireless warns about RF near grows: {}", d2.detail);
    }

    /// The shipped seed home's data link (uplink -> server over Cat6) sizes OK (no FAIL).
    #[test]
    fn buildability_seed_home_data_links_are_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let report = home.buildability_report(4.5);
        if let Some(d) = report.checks.iter().find(|c| c.name == "Data links") {
            assert_ne!(d.status, CheckStatus::Fail, "seed data links must size: {}", d.detail);
        }
    }

    /// An electrical LOAD wired only to a battery (no generator on its circuit) FAILS the Power
    /// circuit check -- the operator's "no magic transmission": a battery is storage, not generation.
    #[test]
    fn buildability_power_circuit_flags_an_isolated_load() {
        let mut catalog = BTreeMap::new();
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 1000.0, max_charge_w: 500.0, max_discharge_w: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1 })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0) };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("b1", "batt"), inst("l1", "load")],
            arrays: Vec::new(),
            // load wired to a battery that itself reaches NO generator -> the load can't run.
            connections: vec![MachineConnection { from: "b1".into(), to: "l1".into(), kind: "power".into(), spec: None }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let circuit = home.power_circuit_check(&home.all_instances()).expect("electrical machines -> a circuit check");
        assert_eq!(circuit.status, CheckStatus::Fail, "battery-only load fails: {}", circuit.detail);
        // Now wire a panel onto the same bus -> the load traces to generation -> Pass.
        home.catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 800.0 })));
        home.instances.push(inst("p1", "panel"));
        home.connections.push(MachineConnection { from: "p1".into(), to: "b1".into(), kind: "power".into(), spec: None });
        let circuit = home.power_circuit_check(&home.all_instances()).expect("a circuit check");
        assert_eq!(circuit.status, CheckStatus::Pass, "panel->battery->load now traces: {}", circuit.detail);
    }

    /// A load that reaches a generator only through a junction NODE + power conduit edges still counts
    /// as wired (the check traverses the conduit graph, not just machine-to-machine connections).
    #[test]
    fn buildability_power_circuit_traverses_conduit_nodes() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 80.0, priority: 1 })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0) };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("p1", "panel"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        };
        let nid = home.add_conduit_node((1.0, 1.0, 1.0), "power");
        assert!(home.add_conduit_edge(ConduitEnd::Machine("p1".into()), ConduitEnd::Node(nid.clone()), "power"));
        assert!(home.add_conduit_edge(ConduitEnd::Node(nid), ConduitEnd::Machine("l1".into()), "power"));
        let circuit = home.power_circuit_check(&home.all_instances()).expect("a circuit check");
        assert_eq!(circuit.status, CheckStatus::Pass, "panel->node->load is wired: {}", circuit.detail);
    }

    /// The shipped seed home has a COHERENT, CONNECTED water sim (v0.610, after the adversarial-review
    /// fixes): the cistern + the powered well pump + the irrigation draw + the non-powered household tap
    /// all share ONE plumbing island, and there is a non-powered demand that exceeds the passive rain --
    /// so the cistern actually fills when powered and DRAINS when the grid is cut (the consequence the
    /// Live water card advertises). Guards against a regression to the old inert/disconnected topology.
    #[test]
    fn seed_home_water_topology_is_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let all = home.all_instances();

        // Cistern stores 8000 L; its passive rain inflow is small (less than the household demand).
        let cistern = home.catalog.get("water_tank").expect("a cistern type");
        assert!((cistern.water_capacity_l() - 8000.0).abs() < 1.0, "cistern stores 8000 L");
        let rain = cistern.water_production_lpm();
        // The well pump is the POWERED water source.
        let pump = home.catalog.get("water_pump").expect("a pump type");
        assert!(pump.water_production_lpm() > 0.0, "pump produces water");
        assert!(pump.draws_power(), "pump is power-gated (its water output stops on a power cut)");
        // The household tap is a NON-powered demand that exceeds rain -> drains the cistern when the
        // powered pump stops. This is what makes the consequence visible.
        let household = home.catalog.get("home_water_use").expect("a household-water type");
        let tap = household.water_demand_lpm();
        assert!(tap > 0.0, "household draws water");
        assert!(!household.draws_power(), "household tap is NOT power-gated (taps flow in a blackout)");
        assert!(tap > rain, "non-powered demand {tap} must exceed passive rain {rain} so the cistern can drain");

        // The aeroponic towers no longer each form a standalone water island (HIGH-1 fix): they are
        // electricity-only now (water recirculates), so they are not water-graph members.
        let tower = home.catalog.get("aeroponic_tower_nutrition").expect("a tower type");
        assert!(!tower.is_water_machine(), "towers recirculate -> not standalone water sinks");
        assert!(tower.draws_power(), "towers still need electricity");

        // ONE connected plumbing island for the home's water (not 25). cistern + pump + irrigation +
        // household must share it.
        let islands = home.water_islands(&all);
        let distinct: std::collections::HashSet<u32> = islands.values().copied().collect();
        assert_eq!(distinct.len(), 1, "the seed home's water is ONE island, got {distinct:?}");
        for id in ["cistern_1", "pump_1", "irrigation_1", "household_1"] {
            assert!(islands.contains_key(id), "{id} is on the water graph");
        }
        let i = islands["cistern_1"];
        assert!(["pump_1", "irrigation_1", "household_1"].iter().all(|m| islands[*m] == i),
            "cistern, pump, irrigation, household share one island");
    }

    /// The shipped seed home is a fully-wired network: every load traces to a generator (no FAIL).
    #[test]
    fn buildability_seed_home_power_circuit_is_connected() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let report = home.buildability_report(4.5);
        let circuit = report.checks.iter().find(|c| c.name == "Power circuit").expect("the seed has electrical machines");
        assert_ne!(circuit.status, CheckStatus::Fail, "seed power must be fully wired: {}", circuit.detail);
    }

    /// The shipped seed home's power runs all size to a real copper cable (the reference design must be
    /// buildable, not just internally consistent).
    #[test]
    fn buildability_seed_home_conduits_are_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let report = home.buildability_report(4.5);
        // If the seed has power runs at all, the Conduits check must not FAIL (Pass or Warn is fine).
        if let Some(conduit) = report.checks.iter().find(|c| c.name == "Conduits") {
            assert_ne!(conduit.status, CheckStatus::Fail, "seed conduits must be sizable: {}", conduit.detail);
        }
    }
}
