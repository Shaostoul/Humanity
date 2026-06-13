//! Data-driven machine layout for the 3D home (First-Playable groundwork, v0.427).
//!
//! Pure data (serde) so it compiles under every feature set, the renderer placement
//! that turns these into primitives + connection pipes lives in `lib.rs::load_world`
//! (native only). Source file: `data/machines/home.ron`.
//!
//! Infinite-of-X: machines and the connections between them are DATA, not code. Add a
//! `catalog` type, an `instance`, or a `connection` to the RON and it appears in the
//! world, no Rust change.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// One readout shown on a machine's info card: an icon (by `kind`), a value, and a
/// status that colors the icon. Placeholder/demo data until the machines are wired to
/// the live simulation.
#[derive(Debug, Clone, Deserialize)]
pub struct MachineStat {
    /// "power" | "water" | "storage" | "progress" | "heat" | "fuel" | "nutrient".
    pub kind: String,
    /// Human value, e.g. "120 W", "60%", "idle".
    pub value: String,
    /// "ok" | "warn" | "off" | "low". Colors the icon (green / amber / red-grey / amber).
    pub status: String,
}

/// A machine type: which primitive shape to draw it as, its size, color, display name,
/// and the stat readouts shown on its info card.
#[derive(Debug, Clone, Deserialize)]
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
}

/// One placed machine.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct MachineConnection {
    pub from: String,
    pub to: String,
    /// "power" | "water" | "nutrient" | "fuel" (colors the tube).
    pub kind: String,
}

/// One self-sufficiency loop (energy / water / food / nutrients): whether it closes and
/// the honest story. Rendered as the Home-page closure summary.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct MachineHome {
    pub catalog: HashMap<String, MachineDef>,
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
}
