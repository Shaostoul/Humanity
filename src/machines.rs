//! Data-driven machine layout for the 3D home (First-Playable groundwork, v0.427).
//!
//! Pure data (serde) so it compiles under every feature set, the renderer placement
//! that turns these into primitives + connection pipes lives in `lib.rs::load_world`
//! (native only). Source file: `data/machines/home.ron`.
//!
//! Infinite-of-X: machines and the connections between them are DATA, not code. Add a
//! `catalog` type, an `instance`, or a `connection` to the RON and it appears in the
//! world, no Rust change.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

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
    /// Stat readouts shown on the info card when you are close.
    #[serde(default)]
    pub stats: Vec<MachineStat>,
    /// Electrical role in the live sim (generator / consumer). None = not on the grid.
    #[serde(default)]
    pub power: Option<MachinePower>,
}

/// One placed machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInstance {
    pub id: String,
    pub machine: String,
    /// A homestead room id (see data/blueprints/homestead_layout.ron).
    pub room: String,
    /// (x, y, z) meters from the room center, y up from the floor.
    pub offset: (f32, f32, f32),
}

/// A grid of identical machines, expanded into instances at load time. Lets a dense
/// array (e.g. an indoor garden packed with aeroponic towers) be ONE data line instead
/// of hundreds of hand-typed instances. Infinite-of-X: the array IS the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineArray {
    /// Catalog type to repeat.
    pub machine: String,
    /// Room id to place the grid in.
    pub room: String,
    /// (x, y, z) meters from the room center for the grid's first (row 0, col 0) cell.
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
}

/// Pass / warn / fail verdict for one buildability check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        let header = "// HumanityOS home machine layout. Editable in the construction editor or by\n\
                      // hand. Real-world energy/water/food model: docs/design/self-sufficiency.md.\n\
                      // Design architecture: docs/design/home-design.md.\n\n";
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
    }

    /// Drop every machine (explicit instance + array grid) placed in `room_id`, then prune any
    /// connection whose endpoint no longer resolves. Called when a room is deleted in the
    /// construction editor so its machines do not become orphaned -- invisible in-world (the
    /// renderer skips a machine whose room is gone) AND un-removable through the GUI (you can no
    /// longer select the deleted room to reach them). Returns true if anything was removed, so
    /// the caller knows whether to persist. (v0.522)
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

        BuildabilityReport { checks }
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

    /// Color (rgba) for a connection kind.
    pub fn connection_color(kind: &str) -> [f32; 4] {
        match kind {
            "power" => [0.95, 0.75, 0.15, 1.0],    // amber (energized conduit)
            "water" => [0.20, 0.45, 0.85, 1.0],    // blue (utility/AWWA potable)
            "nutrient" => [0.45, 0.30, 0.16, 1.0], // brown
            "fuel" => [0.55, 0.50, 0.18, 1.0],     // olive (flammable)
            "air" => [0.30, 0.70, 0.85, 1.0],      // cyan (compressed air/gas, ASME safety blue family)
            "waste" => [0.30, 0.36, 0.30, 1.0],    // grey-green (drain/greywater)
            _ => [0.6, 0.6, 0.6, 1.0],
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
            stats: Vec::new(),
            power: None,
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
        };
        let mut home = MachineHome {
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: vec![conn("a", "b"), conn("b", "c"), conn("c", "a")],
            loops: Vec::new(),
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
            }],
            loops: Vec::new(),
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
        };
        let report = home.buildability_report(4.5);
        assert_eq!(report.worst(), CheckStatus::Fail);
        assert!(report.checks.iter().any(|c| c.name == "Power source" && c.status == CheckStatus::Fail));
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
            connections: Vec::new(),
            loops: Vec::new(),
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
            connections: Vec::new(),
            loops: Vec::new(),
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
            connections: vec![MachineConnection { from: "a".into(), to: "ghost".into(), kind: "power".into() }],
            loops: Vec::new(),
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
}
