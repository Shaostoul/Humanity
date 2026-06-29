//! Utility wiring -- connection ports + cables/conduit with REAL specs (v0.604).
//!
//! The operator's rule: power / water / air / data must NOT magically transmit through the air. They
//! travel through CABLES and PLUMBING that have real limits -- volts, watts, amps, gauge (AWG),
//! ampacity, shielded vs unshielded -- and a machine declares physical IN/OUT PORTS by utility (a
//! teleporter needs electricity; an aeroponic tower needs water + electricity; a sink needs hot AND
//! cold water). We start with COPPER, grounded in real NEC ampacity; a later mission upgrades copper
//! to a room-temperature superconductor.
//!
//! Stage 1 (this module): the pure DATA MODEL + the cable registry (`conduits.ron`) + the physics
//! checks (`check_cable`, voltage drop, `awg_to_mm2`). NO `#[cfg(native)]` gate -- this is plain serde
//! + math so it compiles under `relay` too. Wiring it into `MachineDef` ports + the buildability
//! report is the next increment; see `docs/design/utility-wiring.md` + `docs/design/sim-realism-roadmap.md`.
//!
//! `Utility` is a deliberately CLOSED enum (not a data file): each utility has distinct physics
//! (electricity = ampacity + voltage drop; water = flow + pressure), so adding one is a real code
//! decision. Infinite-of-X lives in the conduit CATALOG (`conduits.ron`) and in machine ports.

use serde::{Deserialize, Serialize};

/// A utility that flows through a cable/pipe. Closed on purpose -- distinct physics per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Utility {
    Electricity,
    Water,
    HotWater,
    Air,
    Data,
    Fuel,
    Nutrient,
    Waste,
}

impl Utility {
    /// Lowercase id used in data + matched against the legacy `MachineConnection.kind` strings.
    pub fn id(&self) -> &'static str {
        match self {
            Utility::Electricity => "power",
            Utility::Water => "water",
            Utility::HotWater => "hot_water",
            Utility::Air => "air",
            Utility::Data => "data",
            Utility::Fuel => "fuel",
            Utility::Nutrient => "nutrient",
            Utility::Waste => "waste",
        }
    }
}

/// Which way a port moves its utility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDir {
    /// The machine CONSUMES this utility (a load / sink).
    In,
    /// The machine SUPPLIES this utility (a source).
    Out,
    /// Either way (a bus / junction / battery terminal).
    Bidirectional,
}

/// A physical connection point on a machine: which utility, which direction, an optional electrical
/// load/supply (watts) or flow (L/min), and a local anchor offset so the port has a real SPOT on the
/// machine body you wire a cable to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub utility: Utility,
    pub dir: PortDir,
    /// Short label ("mains in", "hot", "drain"...).
    #[serde(default)]
    pub label: String,
    /// Electrical load (In) or supply (Out) in WATTS. 0 for non-electrical or unspecified.
    #[serde(default)]
    pub watts: f32,
    /// Flow for a fluid/air port in litres/min (0 if N/A).
    #[serde(default)]
    pub flow_lpm: f32,
    /// Local anchor offset (metres, machine-local) -- where the cable/pipe attaches on the body.
    #[serde(default)]
    pub anchor: (f32, f32, f32),
}

impl Port {
    /// An electrical IN port (a load) drawing `watts`.
    pub fn elec_in(watts: f32) -> Port {
        Port { utility: Utility::Electricity, dir: PortDir::In, label: "power in".into(), watts, flow_lpm: 0.0, anchor: (0.0, 0.0, 0.0) }
    }
    /// An electrical OUT port (a source) supplying `watts`.
    pub fn elec_out(watts: f32) -> Port {
        Port { utility: Utility::Electricity, dir: PortDir::Out, label: "power out".into(), watts, flow_lpm: 0.0, anchor: (0.0, 0.0, 0.0) }
    }
    /// An electrical BIDIRECTIONAL port (a battery / bus terminal) rated to `watts`.
    pub fn elec_bidir(watts: f32) -> Port {
        Port { utility: Utility::Electricity, dir: PortDir::Bidirectional, label: "power".into(), watts, flow_lpm: 0.0, anchor: (0.0, 0.0, 0.0) }
    }
    /// A fluid/air IN port for `utility` at `flow_lpm` litres/min.
    pub fn fluid_in(utility: Utility, flow_lpm: f32) -> Port {
        Port { utility, dir: PortDir::In, label: format!("{} in", utility.id()), watts: 0.0, flow_lpm, anchor: (0.0, 0.0, 0.0) }
    }
    /// A fluid/air OUT port for `utility` at `flow_lpm` litres/min.
    pub fn fluid_out(utility: Utility, flow_lpm: f32) -> Port {
        Port { utility, dir: PortDir::Out, label: format!("{} out", utility.id()), watts: 0.0, flow_lpm, anchor: (0.0, 0.0, 0.0) }
    }
}

/// What a conductor is made of. Drives resistance + ampacity + the upgrade path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConductorMaterial {
    Copper,
    Aluminum,
    /// Room-temperature/pressure superconductor -- the late-game upgrade target (near-zero loss).
    Superconductor,
}

/// Build grade -- a home does not need industrial feeders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    Home,
    Commercial,
    Industrial,
}

/// A cable / conduit TYPE from the registry (`data/utilities/conduits.ron`). Real-ish specs so wiring
/// teaches real engineering. For electricity: AWG gauge, ampacity (A), voltage rating, shielded.
#[derive(Debug, Clone, Deserialize)]
pub struct ConduitType {
    pub id: String,
    pub label: String,
    pub utility: Utility,
    pub material: ConductorMaterial,
    /// American Wire Gauge (electrical). 0 for non-electrical.
    #[serde(default)]
    pub awg: i32,
    /// Continuous current the cable can carry, AMPS (NEC-ish for copper). For a pipe: 0.
    #[serde(default)]
    pub ampacity_a: f32,
    /// Insulation/voltage rating, VOLTS.
    #[serde(default)]
    pub voltage_max: f32,
    /// Resistance per metre (ohm/m) of one conductor -- for voltage-drop. ~0 for a superconductor.
    #[serde(default)]
    pub ohm_per_m: f32,
    /// Pipe inner diameter (mm) for a fluid conduit; 0 for electrical.
    #[serde(default)]
    pub diameter_mm: f32,
    /// Max continuous flow (L/min) for a fluid conduit; 0 for electrical.
    #[serde(default)]
    pub flow_max_lpm: f32,
    #[serde(default)]
    pub shielded: bool,
    pub grade: Grade,
    /// Cost per metre (cred-equivalent) -- so the auto-picker chooses the cheapest cable that fits.
    #[serde(default)]
    pub cost_per_m: f32,
    pub note: String,
}

/// The conduit/cable registry, parsed once + embedded (same pattern as the other registries).
pub fn conduit_types() -> &'static [ConduitType] {
    static REG: std::sync::OnceLock<Vec<ConduitType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../data/utilities/conduits.ron");
        match ron::from_str::<Vec<ConduitType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("conduits.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a conduit type by id.
pub fn conduit_type(id: &str) -> Option<&'static ConduitType> {
    conduit_types().iter().find(|c| c.id == id)
}

/// AWG -> cross-section area (mm^2). The standard geometric relation: each 6 gauges ~ halves the area;
/// AWG 0 ('1/0' family approximated) anchors near 53.5 mm^2. Used for display + future resistance calc.
pub fn awg_to_mm2(awg: i32) -> f32 {
    // d(mm) = 0.127 * 92^((36-awg)/39); area = pi/4 * d^2.
    let d_mm = 0.127 * 92f32.powf((36 - awg) as f32 / 39.0);
    std::f32::consts::PI * 0.25 * d_mm * d_mm
}

/// Verdict of checking a cable against a real electrical load.
#[derive(Debug, Clone, PartialEq)]
pub enum CableVerdict {
    /// Comfortably within ampacity + acceptable voltage drop (<3%).
    Pass,
    /// Carries it but marginal (near ampacity, or 3-5% drop) -- a real install would up-size.
    Warn,
    /// Over ampacity or >5% drop / over-voltage -- unsafe; would trip/overheat.
    Fail,
}

/// The result of an electrical cable check: verdict + the computed numbers + a human reason.
#[derive(Debug, Clone)]
pub struct CableCheck {
    pub verdict: CableVerdict,
    pub amps: f32,
    pub drop_pct: f32,
    pub reason: String,
}

/// Check an ELECTRICAL cable against a load: amps = watts / volts; voltage drop over the round-trip
/// length = I * R * 2L. Pass if amps <= 80% ampacity (NEC continuous-load derate) AND drop < 3%;
/// Warn up to ampacity / 5% drop; Fail beyond, or if the cable's voltage rating is below the circuit.
pub fn check_cable(cable: &ConduitType, load_watts: f32, volts: f32, length_m: f32) -> CableCheck {
    if cable.utility != Utility::Electricity {
        return CableCheck { verdict: CableVerdict::Fail, amps: 0.0, drop_pct: 0.0, reason: format!("{} is not an electrical cable", cable.label) };
    }
    let volts = volts.max(1.0);
    let amps = load_watts.max(0.0) / volts;
    // Round-trip voltage drop: out + return conductor.
    let v_drop = amps * cable.ohm_per_m * 2.0 * length_m.max(0.0);
    let drop_pct = (v_drop / volts) * 100.0;
    let ampacity = cable.ampacity_a.max(0.001);
    let over_voltage = cable.voltage_max > 0.0 && volts > cable.voltage_max;
    let verdict = if over_voltage || amps > ampacity || drop_pct > 5.0 {
        CableVerdict::Fail
    } else if amps > ampacity * 0.8 || drop_pct > 3.0 {
        CableVerdict::Warn
    } else {
        CableVerdict::Pass
    };
    let reason = if over_voltage {
        format!("{:.0} V exceeds the cable's {:.0} V rating", volts, cable.voltage_max)
    } else {
        format!("{:.1} A on a {:.0} A cable ({}), {:.1}% drop over {:.1} m", amps, ampacity, cable.label, drop_pct, length_m)
    };
    CableCheck { verdict, amps, drop_pct, reason }
}

/// Pick the CHEAPEST electrical cable in the registry that PASSES the load (the auto-picker the
/// buildability report uses). Returns None if nothing in the catalog can carry it.
pub fn cheapest_cable_for(load_watts: f32, volts: f32, length_m: f32) -> Option<&'static ConduitType> {
    conduit_types()
        .iter()
        .filter(|c| c.utility == Utility::Electricity)
        .filter(|c| check_cable(c, load_watts, volts, length_m).verdict == CableVerdict::Pass)
        .min_by(|a, b| a.cost_per_m.partial_cmp(&b.cost_per_m).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_parses_with_copper_and_the_superconductor() {
        let types = conduit_types();
        assert!(!types.is_empty(), "conduits.ron should parse");
        for id in ["cu_awg14", "cu_awg12", "cu_awg10", "cu_awg6_ind", "sc_room_temp"] {
            assert!(conduit_type(id).is_some(), "missing conduit type {id}");
        }
        // The superconductor is near-lossless + the highest ampacity.
        let sc = conduit_type("sc_room_temp").unwrap();
        assert!(sc.ohm_per_m < 1e-4, "superconductor is ~lossless");
        assert!(sc.material == ConductorMaterial::Superconductor);
    }

    #[test]
    fn awg_areas_are_realistic() {
        // AWG 12 ~ 3.31 mm^2, AWG 10 ~ 5.26 mm^2 (within a few %).
        assert!((awg_to_mm2(12) - 3.31).abs() < 0.2, "awg12 ~3.31 mm2, got {}", awg_to_mm2(12));
        assert!((awg_to_mm2(10) - 5.26).abs() < 0.3, "awg10 ~5.26 mm2, got {}", awg_to_mm2(10));
        assert!(awg_to_mm2(6) > awg_to_mm2(10), "lower gauge = thicker");
    }

    #[test]
    fn check_cable_passes_warns_and_fails() {
        let awg14 = conduit_type("cu_awg14").unwrap(); // ~15 A
        // 120 W @ 120 V = 1 A -> comfortable Pass on a short run.
        assert_eq!(check_cable(awg14, 120.0, 120.0, 5.0).verdict, CableVerdict::Pass);
        // 2000 W @ 120 V = 16.7 A -> over the 15 A ampacity -> Fail.
        assert_eq!(check_cable(awg14, 2000.0, 120.0, 5.0).verdict, CableVerdict::Fail);
        // A very long run pushes voltage drop past 5% even at a modest load -> Fail.
        let long = check_cable(awg14, 1200.0, 120.0, 120.0);
        assert!(matches!(long.verdict, CableVerdict::Warn | CableVerdict::Fail), "long run derates: {:?}", long);
    }

    #[test]
    fn auto_picker_returns_cheapest_that_fits_and_scales_with_load() {
        // A light load picks a thin (cheap) copper cable.
        let light = cheapest_cable_for(120.0, 120.0, 5.0).expect("a cable fits 1 A");
        // A heavy load needs a thicker (more expensive) cable -- and the light one must NOT suffice.
        let heavy = cheapest_cable_for(5000.0, 240.0, 5.0).expect("a cable fits ~21 A");
        assert!(heavy.ampacity_a >= light.ampacity_a, "heavier load -> >= ampacity cable");
        assert_eq!(check_cable(light, 5000.0, 240.0, 5.0).verdict != CableVerdict::Pass, true, "the light cable can't carry the heavy load");
    }

    #[test]
    fn superconductor_carries_a_load_that_copper_cant() {
        // The upgrade payoff (v0.616): a big load over a long run -- 10 kW @ 240 V = 41.7 A over 100 m.
        let awg14 = conduit_type("cu_awg14").unwrap();
        let sc = conduit_type("sc_room_temp").unwrap();
        assert_eq!(check_cable(awg14, 10000.0, 240.0, 100.0).verdict, CableVerdict::Fail, "thin copper fails the big run");
        assert_eq!(check_cable(sc, 10000.0, 240.0, 100.0).verdict, CableVerdict::Pass, "the room-temp superconductor carries it");
    }

    #[test]
    fn a_pipe_is_not_an_electrical_cable() {
        // (Guards the utility-mismatch path even before pipes are wired.)
        for c in conduit_types().iter().filter(|c| c.utility != Utility::Electricity) {
            assert_eq!(check_cable(c, 100.0, 120.0, 1.0).verdict, CableVerdict::Fail);
        }
    }
}
