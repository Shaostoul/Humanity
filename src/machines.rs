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
    /// `average_watts` (2026-09-27): what the panel delivers to the home's loads
    /// averaged over a year at the home's site, after its system losses and the
    /// batteries' round trip (a PVWatts-class figure, quoted beside it in the
    /// data). The static power meters credit it for the day; absent, they fall
    /// back to `peak_watts` x the sun-hours they are given, which counts no
    /// losses at all. The live sim still follows the sun with `peak_watts`.
    Solar {
        peak_watts: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        average_watts: Option<f32>,
    },
    /// Steady generator: constant output while active. `fuel_lph` litres/hour
    /// of drum fuel burned while RUNNING (v0.733) — 0 (the serde default) = a
    /// free source that's always on, whose `watts` is therefore what it makes
    /// averaged over a day (a wind turbine's site average since 2026-09-27; its
    /// nameplate sits on its port, which sizes the cable); > 0 = a backstop
    /// genset that only runs when its island is short AND the batteries are
    /// low, drawing from the machine's own Container (empty drum = no watts).
    /// The static meters never count a backstop as what the home makes: they
    /// show what it can add while it runs, apart (2026-09-27).
    Generator {
        watts: f32,
        #[serde(default)]
        fuel_lph: f32,
    },
    /// Power draw. `priority` 1 = critical (shed last), 5 = optional (shed first).
    /// `idle_watts` (2026-09-26): a work station (stove, oven, electronics
    /// bench) draws `watts` only while a craft runs at it and `idle_watts`
    /// otherwise. Absent = a steady load that always draws `watts`.
    /// `average_watts` (2026-09-27): a machine whose controller runs it below
    /// its full `watts` (an air handler on its speed curve, a fan or a
    /// humidifier holding a setpoint, a scrubber cycling) averages this over a
    /// day in its home at full planting, as the model measures it
    /// (farming::life_support_tests pins each figure to the measured balance).
    /// The static power meters charge it for the day instead of `watts`.
    /// Absent = the meters charge its full draw (or its idle draw).
    Consumer {
        watts: f32,
        priority: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        idle_watts: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        average_watts: Option<f32>,
    },
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
    /// What a line LEAVING this machine carries, by connection kind, where that is more specific
    /// than the kind's own group (2026-10-04, pipe marking review): water leaving the purifier is
    /// `potable_water`, water leaving an air handler's coil `condensate`. Each value is a content
    /// id the marking schemes mark (data/piping/marking_schemes.ron). A kind with no entry carries
    /// its group alone (a `water` line is marked fresh water), so a line is never marked more
    /// specifically than the machine it leaves says it is (`MachineHome::line_content`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outlet_media: BTreeMap<String, String>,
    /// Economy automation (v0.663): a recipe id from `data/recipes.csv` this machine runs
    /// CONTINUOUSLY against the home inventory whenever the inputs are in stock (the smelter
    /// auto-runs `smelt_iron`, the workbench `craft_hammer`). Spawns an `AutoRefine` ECS
    /// component; `CraftingSystem` does the rest. None = a machine with no auto production.
    #[serde(default)]
    pub auto_recipe: Option<String>,
    /// This machine is the home's garden irrigation (2026-09-25): while it is
    /// powered it waters every grow area and draws the plants' real daily
    /// litres from its plumbing island. With none powered, crops dry out
    /// unless watered by hand. Spawns an `Irrigator` marker.
    #[serde(default)]
    pub irrigates: bool,
    /// With `auto_recipe`: stop while this many of its first output are on
    /// hand (2026-09-26; `AutoRefine::keep`). None = run whenever it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_keep: Option<u32>,
    /// A bulk vessel (silo, fuel drum) shows its fill as a level gauge above
    /// it, like the water tanks (2026-09-26, engine::stock_piles). Furniture
    /// that holds goods (shelves, drawers) does not.
    #[serde(default)]
    pub level_gauge: bool,
    /// This machine is an electric grow light (2026-09-26): while it is
    /// powered, it lights the grow machines nearest it, as much canopy as its
    /// watts serve (farming::lighting), so green crops there keep growing
    /// after the sun sets (outdoor fields have only the sun).
    /// It needs a `Consumer` power role: an unpowered light gives no light.
    /// Spawns a `GrowLight` marker. See `farming::light_growth_rate`.
    #[serde(default)]
    pub lights_crops: bool,
    /// This machine is a bumblebee hive (2026-09-26): its bees pollinate the
    /// flowers of the indoor grow areas around it, as far as one colony's
    /// cited floor area reaches (farming::pollination,
    /// data/garden/pollination.ron). Spawns a `PollinatorHive` marker.
    #[serde(default)]
    pub pollinates_crops: bool,
    /// This machine is an exhaust fan (2026-09-26): while it is powered it
    /// exchanges the air of the grow room it stands in with the home's air,
    /// up to this many m3 an hour at full speed, which is what carries off
    /// the water the crops breathe out (farming::humidity,
    /// data/garden/humidity.ron). 0 = not a fan. Spawns a `Ventilator`.
    #[serde(default)]
    pub ventilation_m3_h: f32,
    /// With `ventilation_m3_h`: the fan is switched by a CO2 controller
    /// (2026-09-27), on while the air it stands in (a grow room's, or a
    /// fruiting tent's) holds more than this many ppm of carbon dioxide, on
    /// for the share of the time that holds it there, off below, drawing its
    /// watts for that share (farming::humidity, data/garden/humidity.ron THE
    /// CO2 FANS). 0 = a humidity-controlled fan.
    #[serde(default)]
    pub co2_setpoint_ppm: f32,
    /// This machine is a humidifier (2026-09-26): while it is powered and the
    /// home has water for the garden, it puts up to this many litres of water
    /// an hour into the air of the grow room it stands in, as much as holds
    /// the room at its setpoint (farming::humidity, data/garden/humidity.ron),
    /// and the litres come out of the home's tanks with the irrigation.
    /// 0 = not a humidifier. Spawns a `Humidifier`; needs a `Consumer` role.
    #[serde(default)]
    pub humidifies_l_h: f32,
    /// This machine is an air handler (2026-09-26, ship life support): while
    /// it is powered its fan pushes up to this many m3 an hour of the air it
    /// stands in (a grow room's, or the home's own) over a cold coil, which
    /// condenses the water out of it and sends it back to the tanks through
    /// its plumbing island (systems::life_support, data/life_support.ron).
    /// 0 = not an air handler. Spawns an `AirHandler`; needs a `Consumer` role
    /// and a water connection to the tanks.
    #[serde(default)]
    pub dehumidifies_m3_h: f32,
    /// This machine is a CO2 scrubber (2026-09-26, ship life support): while it
    /// is powered it takes carbon dioxide out of the air it stands in, up to
    /// this many kg a day at its rated inlet concentration (data/life_support.ron)
    /// and less in thinner air, and vents it overboard. 0 = not a scrubber.
    /// Spawns a `Co2Scrubber`; needs a `Consumer` role.
    #[serde(default)]
    pub scrubs_co2_kg_day: f32,
    /// Typed-container archetype id from `data/containers/types.csv` (v0.728,
    /// "containers show contents"): a grain silo IS a `grain_silo_bin`, the
    /// fuel refinery a `steel_fuel_drum`. Spawns a `Container` ECS component
    /// (volume-capped, content-class typed — see containers.rs) so the walk-up
    /// card can show live contents + fill. None = not a material container.
    #[serde(default)]
    pub container_type: Option<String>,
    /// What this machine serves when the player presses E at it, by the names a built
    /// piece's blueprint uses (`construction::uses::StructureUse::from_provides`; first-hour
    /// audit F5, 2026-10-04): `Some("rest")` is somewhere to sleep, so the bedroom's bed
    /// sleeps you exactly as a bed you built does, where E only opened its card. Only `rest`
    /// acts at a machine (a machine's storage is used through its card). None: E opens the
    /// card. See `engine::built_uses::machine_use`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<String>,
    /// GLB model path (v0.734, docs/game/model-pipeline.md): rendered instead
    /// of the primitive shape when set. Resolved against the DATA dir first
    /// ("models/x.glb"), then the dev repo root ("assets/models/x.glb").
    /// Meters, Y-up, first mesh's first primitive, indexed triangles. The
    /// primitive stays the fallback when the file is missing or malformed —
    /// a bad model never blanks the machine.
    #[serde(default)]
    pub model: Option<String>,
    /// An in-world SCREEN on one face of this machine (in-world screens,
    /// rung 2; docs/design/in-world-screens.md): a flat display that shows a
    /// native app page the player can look at, click, scroll and type into.
    /// `None` for every machine that is not a display. A wall screen, a desk
    /// monitor and a console are all "a box with a screen on its front".
    #[serde(default)]
    pub screen: Option<ScreenDef>,
    /// An in-game CAMERA on this machine (in-world screens, rung 3): the
    /// machine is a camera post whose head looks along `(yaw_deg, pitch_deg)`
    /// with a `fov_deg` lens, and a screen whose source is
    /// `camera:<this instance's id>` shows what it sees. Yaw is about the
    /// vertical axis, 0 = the body's front (-Z at rotation 0), and the placed
    /// instance's `rotation` ADDS to it, so turning the post in the editor
    /// turns the camera. Pitch: positive looks up, negative down (a security
    /// camera on a post usually looks a little down). `None` for every
    /// machine that is not a camera. See `PlacedMachine::camera_pose`.
    #[serde(default)]
    pub camera: Option<(f32, f32, f32)>,
    /// The pixel size `(width, height)` a camera on this machine RENDERS at:
    /// the surface a `camera:<id>` screen is resized to on its first picture,
    /// whatever the wall's own `screen.px` says. The cost of a camera view
    /// is per pixel (the 2026-09-18 measurement: 17 ms for a 1280 x 720
    /// re-render in a 10 fps room, a full extra scene pass), so this is the
    /// one knob that makes a camera cheap: the default 640 x 360 is a
    /// quarter of the wall's pixels for roughly a quarter of the cost, and
    /// the wall quad's physical size is unchanged, the sampler scales the
    /// picture up. Only read for a machine that has a `camera`.
    #[serde(default = "default_camera_px")]
    pub camera_px: (u32, u32),
}

/// The default camera render size (see `MachineDef::camera_px`).
pub const CAMERA_PX_DEFAULT: (u32, u32) = (640, 360);

fn default_camera_px() -> (u32, u32) {
    CAMERA_PX_DEFAULT
}

/// The resolved world pose of a placed camera machine (rung 3): where the
/// lens is and which way it looks, in the HOME frame the machine bodies are
/// drawn in. Built by `PlacedMachine::camera_pose`; the engine's camera
/// screen provider turns it into a renderer `Camera`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    /// The lens position: the machine's top minus a small margin (the head
    /// sits at the top of the post), pushed to the FRONT face so the post's
    /// own body is behind the near plane and never blocks the view.
    pub position: (f32, f32, f32),
    /// Look yaw in degrees about the vertical axis, same convention as the
    /// machine `rotation` (0 = the body's front, -Z; 90 turns the front to
    /// -X): the def's yaw plus the instance's rotation.
    pub yaw_deg: f32,
    /// Look pitch in degrees, clamped to `CAMERA_PITCH_LIMIT_DEG` either way
    /// so a data typo can never point the lens past straight up or down
    /// (which would flip the view basis).
    pub pitch_deg: f32,
    /// Vertical field of view in degrees, clamped to a lens that can exist.
    pub fov_deg: f32,
    /// The pixel size the view is rendered at (`MachineDef::camera_px`,
    /// each side clamped to at least 1 so a zero in data cannot make a
    /// zero-sized texture).
    pub px: (u32, u32),
}

/// The most a camera may look up or down, in degrees. 89 leaves the look
/// basis well-defined (straight up would make "which way is up" ambiguous).
pub const CAMERA_PITCH_LIMIT_DEG: f32 = 89.0;
/// The narrowest and widest lens a camera def may ask for, in degrees.
pub const CAMERA_FOV_RANGE_DEG: (f32, f32) = (10.0, 150.0);
/// How far below the machine's top the lens sits, in metres: the head of a
/// camera post is the top of its body, and the lens is a little below the
/// very top edge.
pub const CAMERA_LENS_BELOW_TOP_M: f32 = 0.05;

impl PlacedMachine {
    /// Where this machine's camera looks from, when it has one. `None` for
    /// a machine whose def has no `camera`.
    pub fn camera_pose(&self) -> Option<CameraPose> {
        let (yaw_def, pitch_def, fov_def) = self.camera?;
        let yaw_deg = yaw_def + self.rotation;
        // The lens sits on the front face so the body is behind it. The
        // front face at rotation 0 is -Z; the yaw turns it about Y the same
        // way the renderer turns the body mesh (`Quat::from_rotation_y`).
        let yaw = yaw_deg.to_radians();
        let front = (-yaw.sin(), -yaw.cos());
        let half_depth = self.size.2 * 0.5;
        let position = (
            self.pos.0 + front.0 * half_depth,
            (self.top_y - CAMERA_LENS_BELOW_TOP_M).max(self.pos.1),
            self.pos.2 + front.1 * half_depth,
        );
        Some(CameraPose {
            position,
            yaw_deg,
            pitch_deg: pitch_def.clamp(-CAMERA_PITCH_LIMIT_DEG, CAMERA_PITCH_LIMIT_DEG),
            fov_deg: fov_def.clamp(CAMERA_FOV_RANGE_DEG.0, CAMERA_FOV_RANGE_DEG.1),
            px: (self.camera_px.0.max(1), self.camera_px.1.max(1)),
        })
    }
}

/// What a machine's screen shows and how it sits on the body (rung 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenDef {
    /// What the screen shows, as a source string parsed by
    /// `gui::screen_surface::ScreenSource::parse`: a page id from
    /// `gui::dispatch::page_id` ("inventory", "tasks", "chat", "watch", ...,
    /// optionally written "page:inventory"), or "watch:<stream id>" for a live
    /// stream, "camera:<machine instance id>" for an in-game camera,
    /// "video:<path>" for a clip, "web:<url>" for the readable web. An unknown
    /// string logs a warning and shows a notice screen; it never fails the load.
    pub source: String,
    /// Pixel size of the offscreen page render (width, height). Fixed per
    /// def so a screen's cost is known: 1280 x 720 is a wall screen, 1024 x
    /// 600 a desk monitor. The page is drawn at one egui point per pixel.
    pub px: (u32, u32),
    /// Which face of the box carries the screen. "front" is the box's -Z
    /// face at rotation 0 (glTF forward, docs/game/model-pipeline.md),
    /// rotated by the instance yaw about Y like the body. Also "back" (+Z),
    /// "left" (+X, the viewer's left when facing the front), "right" (-X)
    /// and "top" (+Y, read from the front). Unknown = front, with a warning.
    #[serde(default = "default_screen_face")]
    pub face: String,
    /// Bezel width in metres: the display is inset this far from the face's
    /// edges on every side, so the body's colour frames the picture.
    #[serde(default)]
    pub bezel_m: f32,
    /// Display brightness, the material's emissive strength. 1.0 = the page
    /// as drawn; above 1 glows against a dim room, below 1 dims it.
    #[serde(default = "default_screen_brightness")]
    pub brightness: f32,
}

pub fn default_screen_face() -> String {
    "front".to_string()
}

pub fn default_screen_brightness() -> f32 {
    1.0
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
            Some(MachinePower::Solar { peak_watts, .. }) => vec![Port::elec_out(*peak_watts)],
            Some(MachinePower::Generator { watts, .. }) => vec![Port::elec_out(*watts)],
            Some(MachinePower::Consumer { watts, .. }) => vec![Port::elec_in(*watts)],
            Some(MachinePower::Battery { max_discharge_w, .. }) => vec![Port::elec_bidir(*max_discharge_w)],
            None => Vec::new(),
        }
    }

    /// One of ship life support's air machines (2026-09-27): an air handler or
    /// a CO2 scrubber, the machines the Settings' Ship life support mode puts on
    /// the station's own plant (Station-supplied) or on the home's grid
    /// (Realistic). systems::life_support draws them the same way.
    pub fn is_ship_life_support(&self) -> bool {
        self.dehumidifies_m3_h > 0.0 || self.scrubs_co2_kg_day > 0.0
    }

    /// The watts this machine takes from the home's grid AVERAGED over a day,
    /// for the static power meters (2026-09-27; they charged every consumer its
    /// full draw for 24 hours, so an air handler read 7.8 kWh a day whatever it
    /// did): none for storage, none for a ship life support machine the
    /// station's plant powers (`basis`), a controller-driven machine's measured
    /// `average_watts`, a work station's idle draw (its working draw is
    /// `working_extra_watts`, charged by the hour it works), a grow light
    /// (`grow_light`) for its timer's `GROW_LIGHT_DUTY_HOURS`, and anything else
    /// its full electrical load.
    pub fn average_load_watts(&self, basis: MeterBasis, grow_light: bool) -> f32 {
        if matches!(self.power, Some(MachinePower::Battery { .. })) {
            return 0.0;
        }
        if self.is_ship_life_support() && !basis.life_support_on_grid {
            return 0.0;
        }
        let full = self.electrical_load_watts();
        match &self.power {
            Some(MachinePower::Consumer { average_watts: Some(a), .. }) => a.max(0.0),
            Some(MachinePower::Consumer { idle_watts: Some(i), .. }) => i.max(0.0),
            _ if grow_light => full * GROW_LIGHT_DUTY_HOURS / 24.0,
            _ => full,
        }
    }

    /// What a work station draws over its idle draw while a craft runs at it
    /// (2026-09-27): its `watts` less its `idle_watts`; 0 for anything that is
    /// not a work station.
    pub fn working_extra_watts(&self) -> f32 {
        match &self.power {
            Some(MachinePower::Consumer { watts, idle_watts: Some(i), .. }) => (watts - i).max(0.0),
            _ => 0.0,
        }
    }

    /// The watts this machine MAKES for the home on its own, averaged over a
    /// day, for the static power meters (2026-09-27; they counted every
    /// generator at its `watts` for 24 hours, a backstop genset included, and
    /// every panel at its peak x the sun-hours with no losses): a panel's
    /// `average_watts` at the home's site (or, lacking one, `peak_watts` x
    /// `sun_hours` / 24), a fuel-free generator's `watts` (its day's average),
    /// and nothing for a backstop genset, which only makes power while fuel is
    /// burned for it (`backstop_watts`).
    pub fn average_supply_watts(&self, sun_hours: f32) -> f32 {
        match &self.power {
            Some(MachinePower::Solar { average_watts: Some(a), .. }) => a.max(0.0),
            Some(MachinePower::Solar { peak_watts, .. }) => peak_watts.max(0.0) * sun_hours.clamp(0.0, 24.0) / 24.0,
            Some(MachinePower::Generator { watts, fuel_lph }) if *fuel_lph <= 0.0 => watts.max(0.0),
            _ => 0.0,
        }
    }

    /// A backstop genset's output while it runs, and the litres of fuel an hour
    /// it burns doing so (2026-09-27): what it CAN add to the home, which the
    /// meters show apart from what the home makes. (0, 0) for anything else.
    pub fn backstop_watts(&self) -> (f32, f32) {
        match &self.power {
            Some(MachinePower::Generator { watts, fuel_lph }) if *fuel_lph > 0.0 => (watts.max(0.0), *fuel_lph),
            _ => (0.0, 0.0),
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

/// The default ship zone a machine belongs to when its data predates multi-zone ships (v0.754,
/// ship-superstructure increment A): the player's home. Every existing home.ron parses unchanged.
pub fn default_machine_zone() -> String {
    "home".to_string()
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
    /// - ShipStructure zone home: x/z are ZONE-LOCAL metres from the named `zone`'s min corner
    ///   (since increment 1a; they were absolute world metres from v0.754), kept inside that
    ///   zone's footprint on resolve (`zone_world_pos`); y is up from the ZONE's floor (its
    ///   origin y), so a machine on a raised deck sits on that deck.
    /// - legacy AABB-room ship layout: (x, y, z) RELATIVE to the room center, y up from the floor.
    pub offset: (f32, f32, f32),
    /// Yaw rotation in DEGREES about the vertical (Y) axis (v0.633). 0 = unrotated. Lets a box-shaped
    /// machine (a teleporter, a server) face a chosen direction. serde-default so every existing
    /// home.ron + array-expanded cell is unchanged.
    #[serde(default)]
    pub rotation: f32,
    /// The SHIP ZONE this machine lives in (v0.754, ship-superstructure increment A): a
    /// `ShipStructure` zone id. Placement clamps the machine into THIS zone's footprint at that
    /// zone's origin. serde-defaults to "home" so every pre-zone home.ron is unchanged.
    #[serde(default = "default_machine_zone")]
    pub zone: String,
    /// What THIS screen shows, when the def is a display (in-world screens,
    /// rung 2): a source string (see `ScreenDef::source`) that overrides the
    /// def's `screen.source`. The catalog says "a 1280 x 720 wall screen"; the
    /// instance says "this one shows the tasks board". `None` = the def's
    /// default page; ignored on a machine with no screen.
    #[serde(default)]
    pub screen_source: Option<String>,
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
    /// First (row 0, col 0) cell position -- same dual meaning as `MachineInstance.offset`:
    /// ZONE-LOCAL x/z in a ship zone (since increment 1a), room-center-relative in a legacy ship
    /// layout. `spacing` is a local step in both. (v0.538)
    pub origin: (f32, f32, f32),
    /// Number of rows (stepped along +z) and columns (stepped along +x).
    pub rows: u32,
    pub cols: u32,
    /// (x_step, z_step) meters between adjacent cells.
    pub spacing: (f32, f32),
    /// Id prefix for the generated instances (e.g. "tower" -> "tower_0", "tower_1", ...).
    pub id_prefix: String,
    /// The SHIP ZONE the whole grid lives in (v0.754); every expanded cell inherits it. Defaults
    /// to "home" so every pre-zone home.ron is unchanged.
    #[serde(default = "default_machine_zone")]
    pub zone: String,
}

/// A pipe / cable / tube between two machines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConnection {
    pub from: String,
    pub to: String,
    /// "power" | "water" | "nutrient" | "fuel": what the line carries, which picks its conduit
    /// (ship::conduits) and its marker bands (ship::pipe_marking).
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
    /// Marks the FOOD loop and states its demand in kcal a day (2026-09-26). Its
    /// supply is then the home's computed food (`MachineHome::grown_kcal_per_day`), and
    /// whether it closes is computed against this, not read from `closes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food_demand_kcal: Option<f32>,
}

impl HomeLoop {
    /// Does this loop close? The food loop (`food_demand_kcal`) closes when the home's
    /// computed food meets its demand; every other loop by its authored `closes`.
    pub fn closes_given(&self, grown_kcal_per_day: f32) -> bool {
        self.food_demand_kcal.map_or(self.closes, |d| grown_kcal_per_day >= d)
    }
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
    /// The SHIP's machine file this household shares (increment 1a of
    /// docs/design/ship-homes-and-logistics.md), named relative to this file's folder
    /// (`Some("ship.ron")`). `load` merges that file's rows in, and `save` writes them back
    /// there instead of into this file: a row belongs to the ship file when its `zone` is not
    /// "home", and a connection when either end is such a row. data/machines/home.ron names it
    /// because its battery bank powers the Commons' machines and its loops count them (how the
    /// household was modelled before the split); home_solo.ron does not, so it loads without
    /// them, as it always has. None (the default) = a self-contained file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ship_machines: Option<String>,
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
    /// COMPUTED, never saved (2026-09-26): each grow machine type's food a day, from
    /// the crops it grows (`systems::grow_machines`). Filled by `load`; read through
    /// `stats_for`, which makes it the food line of every card that shows the machine.
    #[serde(skip)]
    pub grown: BTreeMap<String, crate::systems::grow_machines::GrownFood>,
    /// COMPUTED, never saved: what `load` merged in from the `ship_machines` file, and whether
    /// those rows may be edited right now (see `ShipPart`).
    #[serde(skip)]
    pub ship_part: ShipPart,
}

/// The ship's share of a merged machine layout (increment 1a of
/// docs/design/ship-homes-and-logistics.md). Rows and connections are told apart by zone (a
/// row whose `zone` is not "home" is the ship's), but loops and conduit nodes carry no zone, so
/// `load` records which ones came from the ship file and the save sends exactly those back.
#[derive(Debug, Clone, Default)]
pub struct ShipPart {
    /// True outside the Dev play mode. Only a Dev save writes the ship's machine file, so an edit
    /// to a ship row made in any other mode would be dropped on the next load without a word.
    /// While this is set, the ship's rows and every connection or conduit edge touching them are
    /// read-only (`is_locked`): the edit methods refuse them and the editor will not select them.
    /// The construction editor sets it from the play mode every frame it draws
    /// (`gui::pages::construction::sync_ship_machine_lock`), and every path that edits machines
    /// runs with the editor open. False (editable) until then.
    pub locked: bool,
    /// Names of the loops that came from the ship file.
    pub loops: std::collections::BTreeSet<String>,
    /// Ids of the conduit nodes that came from the ship file.
    pub conduit_nodes: std::collections::BTreeSet<String>,
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

/// What the static power meters charge a day (2026-09-27): who powers ship
/// life support. The default is the game's default mode, Station-supplied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeterBasis {
    /// Settings > Gameplay > Ship life support is Realistic: the home's grid
    /// powers the air handlers and the CO2 scrubber, which the meters then
    /// charge at their `average_watts`, and the home runs on what it makes and
    /// stores. False (Station-supplied): the ship's reactor powers them and
    /// feeds the home's grid past what the home makes (`UtilityMeter::reactor`,
    /// 2026-09-27); they are not charged to the home's grid line.
    pub life_support_on_grid: bool,
}

/// A placed machine whose catalog id names it a grow light (`grow_light`, or a
/// `grow_light_` wattage variant): the lights `grow_light_report` meters.
fn is_grow_light(machine: &str) -> bool {
    machine == "grow_light" || machine.starts_with("grow_light_")
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
    /// Power only (2026-09-27): what the home's backstop gensets can add while
    /// they run, in watts, and the litres of fuel an hour they burn doing it.
    /// Never part of `generation`, which is what the home makes on its own.
    pub backstop_watts: f32,
    pub backstop_fuel_lph: f32,
    /// Power only (2026-09-27, `systems::ship_power`): in the Station-supplied
    /// mode, what the ship's reactor supplies a day on the day's average, kWh:
    /// the home grid's use past what the home makes, plus ship life support,
    /// and what the home makes past its use, which goes back to the ship. Both
    /// 0 in the Realistic mode, where the home runs on its own.
    pub reactor: f32,
    pub returned: f32,
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
    UtilityMeter {
        utility: utility.to_string(),
        generation,
        demand,
        self_sufficiency: ss,
        unit: unit.to_string(),
        summary,
        backstop_watts: 0.0,
        backstop_fuel_lph: 0.0,
        reactor: 0.0,
        returned: 0.0,
    }
}

/// The sentence the power meter adds for a home's backstop gensets (2026-09-27):
/// what they can add while they run, apart from what the home makes, and what
/// running them to cover the day's shortfall would take, in hours and fuel.
/// `short_kwh` is the day's demand less what the home makes (0 or less = none).
fn backstop_summary(backstop_w: f32, fuel_lph: f32, short_kwh: f32) -> String {
    let kw = backstop_w / 1000.0;
    let mut s = format!(
        "; not counted: a backstop generator that can add {kw:.1} kW while it runs ({kw:.1} kWh for {fuel_lph:.1} L of fuel an hour)"
    );
    if short_kwh > 0.0 && kw > 0.0 {
        let hours = short_kwh / kw;
        if hours <= 24.0 {
            s.push_str(&format!(
                ", which would run {hours:.1} h a day on {:.1} L of fuel to cover the {short_kwh:.1} kWh the home does not make",
                hours * fuel_lph
            ));
        } else {
            s.push_str(&format!(
                ", which even running all day ({:.1} L of fuel) would make only {:.1} of the {short_kwh:.1} kWh the home does not make",
                24.0 * fuel_lph,
                kw * 24.0
            ));
        }
    }
    s
}

/// Hours/day a grow light runs -- the duty-cycle assumption behind the grow-light power meter.
/// The garden's timer (data/garden/lighting.ron `lamp_photoperiod_h`, 18 h of light a day with
/// the sun's 12) runs a light from sunset to midnight, 6 h, and FarmingSystem draws its power only
/// then; the meter charges the same: kWh/day = fixture watts x 6 h / 1000. (It was 14 h, v0.664,
/// until the timer existed; a test in farming::lighting keeps the two equal.)
pub const GROW_LIGHT_DUTY_HOURS: f32 = 6.0;

/// Verdict of the grow-light power meter (v0.664): where the placed LED grow lights sit against
/// the home's real energy budget. docs/design/self-sufficiency.md calls this meter "the single
/// most honest teaching artifact" -- the sun-lit garden is nearly free to run, but every LED
/// added draws real watt-hours, and lighting staple crops blows the whole home budget by 2.5x-12x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrowLightVerdict {
    /// Green: the lights fit inside the home's FREE headroom (generation minus everything else),
    /// so the home still balances every day.
    WithinHeadroom,
    /// Amber: the lights exceed the free headroom -- the home now runs a daily energy deficit and
    /// eats its battery reserves to keep them on.
    EatingReserves,
    /// Red: the lights ALONE draw more than the whole home generates in a day. No battery rides
    /// this out; it is why real self-sufficient homes grow under the sun.
    ExceedsGeneration,
}

/// The grow-light power meter (v0.664, homestead-solo-design.md gap #5): total LED grow-light
/// draw vs the home's free energy headroom, with a green/amber/red verdict. Only produced when
/// at least one grow light is placed (see `MachineHome::grow_light_report`).
#[derive(Debug, Clone)]
pub struct GrowLightReport {
    /// How many grow-light fixtures are placed.
    pub count: usize,
    /// Their combined fixture wattage (W).
    pub watts: f32,
    /// Their combined daily draw: watts x `GROW_LIGHT_DUTY_HOURS` (kWh/day).
    pub draw_kwh_day: f32,
    /// The home's FREE headroom before the lights: daily generation minus every NON-grow-light
    /// demand (kWh/day). Negative when the home already runs a deficit without any lights.
    pub headroom_kwh_day: f32,
    /// What the home makes on its own a day (kWh/day), as `utility_meters` counts it
    /// (`MachineDef::average_supply_watts` x 24 h; a backstop genset is not counted).
    pub generation_kwh_day: f32,
    pub verdict: GrowLightVerdict,
    /// Plain-language one-liner for the meter row (no jargon, non-blaming, states the numbers).
    pub summary: String,
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

/// One ship zone's footprint, as the machine placer sees it (v0.754, ship-superstructure
/// increment A): the zone id, its world origin (box min corner; y = deck height), and its box
/// size (width, depth, height). Built from `ShipStructure::zone_rects`; plain tuples so this
/// module stays renderer-free + testable.
#[derive(Debug, Clone)]
pub struct ZoneRect {
    pub id: String,
    pub origin: (f32, f32, f32),
    pub size: (f32, f32, f32),
}

/// The zone a machine clamps into: its named zone, else "home", else the first zone -- a
/// DETERMINISTIC fallback chain so a machine whose zone was deleted/renamed still lands somewhere
/// stable (the ship always keeps at least one zone; `ShipStructure::remove_zone` protects "home").
pub fn resolve_zone_rect<'a>(zones: &'a [ZoneRect], id: &str) -> Option<&'a ZoneRect> {
    zones
        .iter()
        .find(|z| z.id == id)
        .or_else(|| zones.iter().find(|z| z.id == "home"))
        .or_else(|| zones.first())
}

/// Where a machine stands in SHIP metres: its ZONE-LOCAL offset (metres from the zone box's
/// min corner; y up from the zone's deck) kept 0.3 m inside the zone's footprint, plus the
/// zone's origin. Zone-local since increment 1a of docs/design/ship-homes-and-logistics.md, so
/// a home's machines ride its plot wherever the plot is (they used to be absolute positions
/// clamped into the zone, which piled every machine against the box edge once a home moved).
/// The one formula `placements` and the world load both use.
pub fn zone_world_pos(zr: &ZoneRect, offset: (f32, f32, f32)) -> (f32, f32, f32) {
    let (ox, oy, oz) = zr.origin;
    let (w, d, _h) = zr.size;
    let x = offset.0.clamp(0.3, (w - 0.3).max(0.3));
    let z = offset.2.clamp(0.3, (d - 0.3).max(0.3));
    (ox + x, oy + offset.1, oz + z)
}

/// The colours the build editor's 3D port gizmos draw a connection kind in, LINEAR light
/// (`MachineHome::gizmo_colours`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoColours {
    /// The port's node sphere: the pipes' band colour.
    pub fill: [f32; 4],
    /// The port's direction arrows and the drag-to-connect line: the band colour, or a light
    /// outline when the band colour is too dark to see against a dark room.
    pub outline: [f32; 4],
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
    /// Yaw degrees about Y (v0.633), carried from `MachineInstance.rotation` so the renderer can orient
    /// the mesh. Array-expanded cells inherit their array's nominal 0 (rotated as a group is a later step).
    pub rotation: f32,
    /// GLB model path from the def (v0.734) — the renderer draws this instead
    /// of the primitive when set (primitive stays the fallback on load error).
    pub model: Option<String>,
    /// The def's screen, carried through so the renderer can build the
    /// display quad and the input ray-test next to the body (rung 2).
    pub screen: Option<ScreenDef>,
    /// The def's camera `(yaw_deg, pitch_deg, fov_deg)`, carried through so
    /// a camera screen can resolve where this post looks from (rung 3; see
    /// `camera_pose`, which folds in `rotation`).
    pub camera: Option<(f32, f32, f32)>,
    /// The def's `camera_px`: the size the camera renders at (see
    /// `MachineDef::camera_px`). Meaningless without a `camera`.
    pub camera_px: (u32, u32),
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

/// Resolve which home design file to load/save, per the operator-configurable
/// `AppConfig::home_variant` setting (Settings page, "Household size"): `"home_solo"` uses
/// the one-person self-sufficient design (`docs/design/homestead-solo-design.md`), anything
/// else -- including an empty/unrecognized/future value -- falls back to the default
/// family-scale `home.ron` rather than panicking on a bad config value. Reads the config
/// file fresh at each call (cheap, startup/world-load/save frequency only, never per-frame).
#[cfg(feature = "native")]
pub fn home_ron_path(data_dir: &Path) -> std::path::PathBuf {
    let variant = crate::config::AppConfig::load().home_variant;
    let filename = if variant == "home_solo" { "home_solo.ron" } else { "home.ron" };
    data_dir.join("machines").join(filename)
}

impl MachineHome {
    /// Load from a RON file. Returns `None` (with a warning) on a missing or invalid
    /// file so the caller can fall back gracefully. When the disk file is absent,
    /// falls back to the EMBEDDED copy by filename (v0.744) — a zero-file fresh
    /// install still gets the full home machine layout.
    ///
    /// A file that names `ship_machines` comes back with that file's rows merged in (see the
    /// field), so every caller sees the same set it saw before the ship's machines moved out.
    pub fn load(path: &Path) -> Option<Self> {
        let mut h = Self::load_file(path)?;
        if let Some(file) = h.ship_machines.clone() {
            let ship_path = path.parent().map(|d| d.join(&file)).unwrap_or_else(|| file.clone().into());
            match Self::load_file(&ship_path) {
                Some(ship) => h.merge_ship_machines(ship),
                None => log::warn!(
                    "machines: {} names ship machines {}, which did not load; the ship's machines are missing",
                    path.display(),
                    ship_path.display()
                ),
            }
        }
        // The home file sits in <data>/machines/, so its data dir is two up.
        if let Some(data_dir) = path.parent().and_then(|p| p.parent()) {
            h.grown = crate::systems::grow_machines::grown_food(&h, data_dir);
        }
        Some(h)
    }

    /// One machine file exactly as written: disk first, else the embedded copy by file name,
    /// no merge and no computed food.
    pub fn load_file(path: &Path) -> Option<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => {
                let rel = path
                    .file_name()
                    .map(|f| format!("machines/{}", f.to_string_lossy()))
                    .unwrap_or_default();
                match crate::embedded_data::get_embedded(&rel) {
                    Some(s) => {
                        crate::embedded_data::note_builtin_copy(&rel, format_args!("{} is absent", path.display()));
                        s.to_string()
                    }
                    None => return None, // absent is fine, distributed builds may omit it
                }
            }
        };
        match ron::from_str::<MachineHome>(&text) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("machines: failed to parse {}: {e}", path.display());
                None
            }
        }
    }

    /// Append a ship machine file's rows, arrays, connections, loops and conduit graph, and any
    /// catalog type it defines that this file does not (this file's definition wins on a clash).
    /// The loops' names and the conduit nodes' ids are recorded in `ship_part`, because they
    /// carry no zone and `split_for_save` must send exactly these back to the ship file.
    fn merge_ship_machines(&mut self, ship: MachineHome) {
        for (k, def) in ship.catalog {
            self.catalog.entry(k).or_insert(def);
        }
        self.instances.extend(ship.instances);
        self.arrays.extend(ship.arrays);
        self.connections.extend(ship.connections);
        self.ship_part.loops.extend(ship.loops.iter().map(|l| l.name.clone()));
        self.loops.extend(ship.loops);
        self.ship_part.conduit_nodes.extend(ship.conduit_nodes.iter().map(|n| n.id.clone()));
        self.conduit_nodes.extend(ship.conduit_nodes);
        self.conduit_edges.extend(ship.conduit_edges);
    }

    /// True when the machine row `id` belongs to the ship and the ship's rows are read-only right
    /// now (`ShipPart::locked`, outside the Dev mode). The editor skips such a machine when
    /// picking and listing, and the edit methods below refuse it.
    pub fn is_locked(&self, id: &str) -> bool {
        self.ship_part.locked && self.ship_machines.is_some() && self.ship_row_ids().contains(id)
    }

    /// Every machine id that is read-only right now (`is_locked`), computed once: empty when the
    /// ship's rows may be edited. For callers that test many ids a frame.
    pub fn locked_ids(&self) -> std::collections::HashSet<String> {
        if self.ship_part.locked && self.ship_machines.is_some() {
            self.ship_row_ids()
        } else {
            Default::default()
        }
    }

    /// True when a connection touches a read-only ship row (`is_locked`).
    pub fn connection_locked(&self, from: &str, to: &str) -> bool {
        let locked = self.locked_ids();
        locked.contains(from) || locked.contains(to)
    }

    /// True when a row in `zone` belongs to the ship's machine file rather than a home's.
    pub fn is_ship_zone(zone: &str) -> bool {
        zone != default_machine_zone()
    }

    /// Every machine id (instances and array cells) that belongs to the ship file.
    fn ship_row_ids(&self) -> std::collections::HashSet<String> {
        self.all_instances()
            .into_iter()
            .filter(|i| Self::is_ship_zone(&i.zone))
            .map(|i| i.id)
            .collect()
    }

    /// Split a merged layout: (the household file's part, the ship file's part). Without a
    /// `ship_machines` link nothing is split off (the whole layout is the household's).
    /// `ship_catalog` is the ship file's own catalog keys, which stay in the ship file.
    fn split_for_save(&self, ship_catalog: &std::collections::BTreeSet<String>) -> (MachineHome, Option<MachineHome>) {
        if self.ship_machines.is_none() {
            return (self.clone(), None);
        }
        let ship_ids = self.ship_row_ids();
        let touches_ship = |c: &MachineConnection| ship_ids.contains(&c.from) || ship_ids.contains(&c.to);
        // Loops and conduit nodes carry no zone: the ones `load` merged in from the ship file go
        // back there (`ship_part`); a conduit edge goes with the ship when either end is a ship
        // row or a ship node, the same rule a connection follows. Everything else is the home's.
        let ship_loop = |l: &HomeLoop| self.ship_part.loops.contains(&l.name);
        let ship_node = |n: &ConduitNode| self.ship_part.conduit_nodes.contains(&n.id);
        let ship_end = |e: &ConduitEnd| match e {
            ConduitEnd::Machine(id) => ship_ids.contains(id),
            ConduitEnd::Node(id) => self.ship_part.conduit_nodes.contains(id),
        };
        let ship_edge = |e: &ConduitEdge| ship_end(&e.from) || ship_end(&e.to);
        let mut home = self.clone();
        home.instances.retain(|i| !Self::is_ship_zone(&i.zone));
        home.arrays.retain(|a| !Self::is_ship_zone(&a.zone));
        home.connections.retain(|c| !touches_ship(c));
        home.catalog.retain(|k, _| !ship_catalog.contains(k));
        home.loops.retain(|l| !ship_loop(l));
        home.conduit_nodes.retain(|n| !ship_node(n));
        home.conduit_edges.retain(|e| !ship_edge(e));
        let ship = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog: self.catalog.iter().filter(|(k, _)| ship_catalog.contains(*k)).map(|(k, v)| (k.clone(), v.clone())).collect(),
            instances: self.instances.iter().filter(|i| Self::is_ship_zone(&i.zone)).cloned().collect(),
            arrays: self.arrays.iter().filter(|a| Self::is_ship_zone(&a.zone)).cloned().collect(),
            connections: self.connections.iter().filter(|c| touches_ship(c)).cloned().collect(),
            loops: self.loops.iter().filter(|l| ship_loop(l)).cloned().collect(),
            conduit_nodes: self.conduit_nodes.iter().filter(|n| ship_node(n)).cloned().collect(),
            conduit_edges: self.conduit_edges.iter().filter(|e| ship_edge(e)).cloned().collect(),
            grown: Default::default(),
        };
        (home, Some(ship))
    }

    /// The catalog keys the linked ship file defines itself (empty when there is none).
    fn ship_catalog_keys(&self, home_path: &Path) -> std::collections::BTreeSet<String> {
        let Some(file) = &self.ship_machines else { return Default::default() };
        home_path
            .parent()
            .map(|d| d.join(file))
            .and_then(|p| Self::load_file(&p))
            .map(|s| s.catalog.into_keys().collect())
            .unwrap_or_default()
    }

    /// Write the ship's part of a merged layout back to the linked ship file next to
    /// `home_path` (the editor calls this only with ShipStructureEditing). Ok(None) when this
    /// layout links no ship file.
    pub fn save_ship_part(&self, home_path: &Path) -> Result<Option<std::path::PathBuf>, String> {
        let Some(file) = &self.ship_machines else { return Ok(None) };
        let (_, ship) = self.split_for_save(&self.ship_catalog_keys(home_path));
        let Some(ship) = ship else { return Ok(None) };
        let path = home_path.parent().map(|d| d.join(file)).unwrap_or_else(|| file.clone().into());
        ship.write_ron(&path)?;
        Ok(Some(path))
    }

    /// Write the layout back to a RON file -- the construction editor's machine save +
    /// the AI's edit target are the SAME file, so an AI-placed machine is player-editable
    /// and vice versa (the home-design parity principle). A header points at the docs;
    /// the body is anonymous-struct RON, matching the seed's style + always re-loadable.
    ///
    /// A layout that links a ship machine file (`ship_machines`) writes only the HOUSEHOLD'S
    /// part here; the ship's rows stay in the ship file (`save_ship_part` writes them, from the
    /// Dev mode only), so a save from any mode can never copy the Commons into a home.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let (home, _) = self.split_for_save(&self.ship_catalog_keys(path));
        home.write_ron(path)
    }

    /// Serialize this layout as it stands, keeping the target file's leading comment header.
    fn write_ron(&self, path: &Path) -> Result<(), String> {
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

    /// The stat readouts to SHOW for a machine type: its catalog stats, with a grow
    /// machine's food line replaced by the computed one (`grown`). Every card, the
    /// construction editor and the garden overview read stats through this, so no two
    /// of them show a different food figure for the same machine.
    pub fn stats_for(&self, machine: &str) -> Vec<MachineStat> {
        let mut stats = self.catalog.get(machine).map(|d| d.stats.clone()).unwrap_or_default();
        if let Some(g) = self.grown.get(machine) {
            stats.retain(|s| s.kind != "food");
            stats.insert(0, g.stat());
        }
        stats
    }

    /// The whole home's computed food, kcal a day: every placed machine (instances and
    /// array cells) at its type's computed figure. The Home page and the garden overview
    /// both show this one number.
    pub fn grown_kcal_per_day(&self) -> f32 {
        self.all_instances().iter().filter_map(|i| self.grown.get(&i.machine)).map(|g| g.kcal_per_day).sum()
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
    ///
    /// A read-only ship row (`is_locked`) is left alone: removing it would only last until the
    /// next load, because the save outside the Dev mode does not write the ship's file.
    pub fn remove_instance(&mut self, id: &str) {
        if self.is_locked(id) {
            return;
        }
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
                    rotation: 0.0,
                    zone: arr.zone.clone(),
                    screen_source: None,
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
    /// A read-only ship node (`end_locked`) is kept.
    pub fn remove_conduit_node(&mut self, id: &str) {
        if self.end_locked(&ConduitEnd::Node(id.to_string())) {
            return;
        }
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
        // Not to a read-only ship row or node (`end_locked`): the save outside Dev would drop it.
        if self.end_locked(&from) || self.end_locked(&to) {
            return false;
        }
        if self.conduit_edges.iter().any(|e| e.from == from && e.to == to) {
            return false;
        }
        self.conduit_edges.push(ConduitEdge { from, to, kind: kind.to_string() });
        true
    }

    /// Remove a conduit edge by index; returns true if removed. (v0.581)
    /// An edge touching a read-only ship row or node (`end_locked`) is kept.
    pub fn remove_conduit_edge(&mut self, idx: usize) -> bool {
        match self.conduit_edges.get(idx) {
            Some(e) if !self.end_locked(&e.from) && !self.end_locked(&e.to) => {
                self.conduit_edges.remove(idx);
                true
            }
            _ => false,
        }
    }

    /// True when a conduit end is a read-only ship row (`is_locked`), or a conduit node that came
    /// from the ship file while the ship part is locked (`ShipPart::locked`).
    fn end_locked(&self, end: &ConduitEnd) -> bool {
        match end {
            ConduitEnd::Machine(id) => self.is_locked(id),
            ConduitEnd::Node(id) => {
                self.ship_part.locked && self.ship_machines.is_some() && self.ship_part.conduit_nodes.contains(id)
            }
        }
    }

    /// Resolve a ConduitEnd to a world anchor (v0.581): a MACHINE uses the SAME low pipe anchor the
    /// renderer uses (placement pos + 0.35 m up); a NODE uses its clamped position. `placements` are
    /// the resolved machine placements; `bounds` is the world (min, max) AABB to clamp nodes into --
    /// the single home box pre-v0.754, the whole ship's zone AABB now (nodes carry no zone id; they
    /// are free graph junctions anywhere on the ship).
    pub fn conduit_anchor(
        &self,
        end: &ConduitEnd,
        placements: &[(String, (f32, f32, f32), f32)], // (id, pos, floor_y)
        bounds: ((f32, f32, f32), (f32, f32, f32)),    // world (min xyz, max xyz)
    ) -> Option<(f32, f32, f32)> {
        let (mn, mx) = bounds;
        match end {
            ConduitEnd::Machine(id) => placements
                .iter()
                .find(|(pid, _, _)| pid == id)
                .map(|(_, pos, floor_y)| (pos.0, floor_y + 0.35, pos.2)),
            ConduitEnd::Node(id) => self.conduit_nodes.iter().find(|n| &n.id == id).map(|n| {
                (
                    n.pos.0.clamp(mn.0 + 0.3, (mx.0 - 0.3).max(mn.0 + 0.3)),
                    n.pos.1.clamp(mn.1 + 0.1, mx.1.max(mn.1 + 0.1)),
                    n.pos.2.clamp(mn.2 + 0.3, (mx.2 - 0.3).max(mn.2 + 0.3)),
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
    /// Also refused: a wire to or from a read-only ship row (`is_locked`), which the save outside
    /// the Dev mode would drop.
    pub fn add_connection(&mut self, from: &str, to: &str, kind: &str) -> bool {
        if from == to || from.is_empty() || to.is_empty() || self.connection_locked(from, to) {
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
    /// A connection touching a read-only ship row (`is_locked`) is kept.
    pub fn remove_connection(&mut self, idx: usize) -> bool {
        match self.connections.get(idx) {
            Some(c) if !self.connection_locked(&c.from, &c.to) => {
                self.connections.remove(idx);
                true
            }
            _ => false,
        }
    }

    /// Remove the connection between two machines, in EITHER direction (v0.626). Lets the viewport
    /// "click a pipe -> Remove" gizmo drop a wire by its endpoints without knowing its list index.
    /// Returns true if a connection was removed. One touching a read-only ship row is kept.
    pub fn remove_connection_between(&mut self, a: &str, b: &str) -> bool {
        if self.connection_locked(a, b) {
            return false;
        }
        let before = self.connections.len();
        self.connections
            .retain(|c| !((c.from == a && c.to == b) || (c.from == b && c.to == a)));
        self.connections.len() != before
    }

    /// Per-utility USAGE METERS for the home (v0.630, grid S2): daily generation vs demand + a self-
    /// sufficiency fraction for power (kWh/day), water (L/day), and data (Mbps). Pure + world-free
    /// (computed from the placed machines' catalog defs), so it runs in the editor + an AI can read it.
    /// Non-punitive: it tells you what you make + use, never penalises consuming. `sun_hours` ~ 4.5.
    /// Power demand is what each machine takes from the home's grid averaged over a day
    /// (`MachineDef::average_load_watts`, 2026-09-27; it was every consumer's full draw for 24 h),
    /// with ship life support on the grid or not by `basis`; a work station's working draw, which
    /// only runs while a craft does, is named in the summary instead of charged for the day.
    /// Power generation is what the home makes on its own averaged over a day
    /// (`MachineDef::average_supply_watts`, 2026-09-27: each panel's yield at the site, a wind
    /// turbine's site average); a backstop genset is shown apart, as what it can add while it runs
    /// (`UtilityMeter::backstop_watts`), because it makes nothing until fuel is burned for it.
    pub fn utility_meters(&self, sun_hours: f32, basis: MeterBasis) -> Vec<UtilityMeter> {
        let (mut supply_watts, mut consumer_watts) = (0.0f32, 0.0f32);
        let (mut backstop_w, mut backstop_lph) = (0.0f32, 0.0f32);
        let mut working_watts = 0.0f32;
        let mut life_support_watts = 0.0f32; // Station-supplied: what the reactor gives ship life support
        let (mut water_prod, mut water_dem) = (0.0f32, 0.0f32);
        let (mut data_sup, mut data_dem) = (0.0f32, 0.0f32);
        for inst in self.all_instances() {
            if let Some(def) = self.catalog.get(&inst.machine) {
                supply_watts += def.average_supply_watts(sun_hours);
                let (w, lph) = def.backstop_watts();
                backstop_w += w;
                backstop_lph += lph;
                // A battery is STORAGE, not demand (fixed v0.664; `average_load_watts` gives it 0).
                consumer_watts += def.average_load_watts(basis, is_grow_light(&inst.machine));
                if def.is_ship_life_support() && !basis.life_support_on_grid {
                    life_support_watts += def.average_load_watts(MeterBasis { life_support_on_grid: true }, false);
                }
                working_watts += def.working_extra_watts();
                water_prod += def.water_production_lpm();
                water_dem += def.water_demand_lpm();
                data_sup += def.data_supply_mbps();
                data_dem += def.data_demand_mbps();
            }
        }
        let mut meters = Vec::new();
        // POWER: kWh/day, what the home makes on its own against what it uses, each averaged
        // over a day; the backstop apart.
        let p_gen = supply_watts * 24.0 / 1000.0;
        let p_dem = consumer_watts * 24.0 / 1000.0;
        if p_gen > 0.0 || p_dem > 0.0 || backstop_w > 0.0 {
            let mut m = make_utility_meter("power", p_gen, p_dem, "kWh/day");
            if working_watts > 0.0 {
                m.summary.push_str(&format!(
                    "; the work stations draw {:.1} kW more while a craft runs at them ({:.1} kWh for each such hour)",
                    working_watts / 1000.0,
                    working_watts / 1000.0
                ));
            }
            if backstop_w > 0.0 {
                m.summary.push_str(&backstop_summary(backstop_w, backstop_lph, p_dem - p_gen));
            }
            m.backstop_watts = backstop_w;
            m.backstop_fuel_lph = backstop_lph;
            // The ship's reactor (2026-09-27, systems::ship_power): the default mode feeds
            // every home island past what the home makes, and ship life support, metered.
            if basis.life_support_on_grid {
                if p_dem > p_gen {
                    m.summary.push_str("; the Realistic mode imports nothing: loads are shed when the batteries run out");
                }
            } else {
                let ls = life_support_watts * 24.0 / 1000.0;
                m.reactor = (p_dem - p_gen).max(0.0) + ls;
                m.returned = (p_gen - p_dem).max(0.0);
                m.summary.push_str(&format!(
                    "; the ship's reactor supplies {:.1} kWh/day ({:.1} to the home's grid, {ls:.1} to ship life support), metered, so nothing browns out",
                    m.reactor,
                    (p_dem - p_gen).max(0.0)
                ));
                if m.returned > 0.0 {
                    m.summary.push_str(&format!("; {:.1} kWh/day goes back to the ship", m.returned));
                }
            }
            meters.push(m);
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

    /// The GROW-LIGHT POWER METER (v0.664): the honest teaching artifact from
    /// docs/design/self-sufficiency.md ("a live grow-light draw vs power budget meter that turns
    /// red the instant any LED is added past the free pump headroom") and homestead-solo-design.md
    /// gap #5. Pure + world-free, computed from the placed machines' catalog defs, so it runs in
    /// the construction editor and an AI can read it before committing a design.
    ///
    /// Returns `None` when no grow light is placed (the meter row only appears once one exists).
    /// A grow light is any placed machine whose catalog id is `grow_light` or starts with
    /// `grow_light_` (data-driven: add wattage variants to the catalog, no code change).
    ///
    /// The math, all in kWh/day like `utility_meters`:
    /// - lights draw = fixture watts x `GROW_LIGHT_DUTY_HOURS` (6 h: the garden timer runs them
    ///   from sunset to its 18 h photoperiod, unlike the 24 h worst-case the generic meter charges
    ///   every consumer).
    /// - free headroom = generation - every NON-grow-light demand (its average draw for 24 h,
    ///   `MachineDef::average_load_watts` under `basis`, matching `utility_meters`); batteries are
    ///   storage, never demand.
    /// - verdict: lights within headroom (green) / past headroom, eating battery reserves daily
    ///   (amber) / lights ALONE exceed the whole home's generation (red).
    pub fn grow_light_report(&self, sun_hours: f32, basis: MeterBasis) -> Option<GrowLightReport> {
        let mut count = 0usize;
        let mut grow_watts = 0.0f32;
        let (mut supply_watts, mut other_watts) = (0.0f32, 0.0f32);
        for inst in self.all_instances() {
            let Some(def) = self.catalog.get(&inst.machine) else { continue };
            // What the home makes on its own, as utility_meters counts it (2026-09-27; a
            // backstop genset is not headroom, it is fuel).
            supply_watts += def.average_supply_watts(sun_hours);
            if is_grow_light(&inst.machine) {
                count += 1;
                grow_watts += def.electrical_load_watts();
            } else {
                // What the rest takes averaged over a day, as utility_meters charges it
                // (2026-09-27; batteries none).
                other_watts += def.average_load_watts(basis, false);
            }
        }
        if count == 0 {
            return None;
        }
        let generation_kwh_day = supply_watts * 24.0 / 1000.0;
        let other_demand_kwh_day = other_watts * 24.0 / 1000.0;
        let headroom_kwh_day = generation_kwh_day - other_demand_kwh_day;
        let draw_kwh_day = grow_watts * GROW_LIGHT_DUTY_HOURS / 1000.0;
        let (verdict, tail) = if draw_kwh_day > generation_kwh_day {
            (
                GrowLightVerdict::ExceedsGeneration,
                format!(
                    "more than the whole home generates ({generation_kwh_day:.1} kWh/day)"
                ),
            )
        } else if draw_kwh_day > headroom_kwh_day {
            (
                GrowLightVerdict::EatingReserves,
                format!(
                    "past the {:.1} kWh/day of free headroom, so every day eats {:.1} kWh out of the battery reserves",
                    headroom_kwh_day.max(0.0),
                    draw_kwh_day - headroom_kwh_day.max(0.0)
                ),
            )
        } else {
            (
                GrowLightVerdict::WithinHeadroom,
                format!(
                    "inside the {headroom_kwh_day:.1} kWh/day of free headroom the home already makes"
                ),
            )
        };
        let plural = if count == 1 { "light" } else { "lights" };
        let summary = format!(
            "{count} grow {plural} draw {draw_kwh_day:.1} kWh/day ({grow_watts:.0} W x {GROW_LIGHT_DUTY_HOURS:.0} h) -- {tail}"
        );
        Some(GrowLightReport {
            count,
            watts: grow_watts,
            draw_kwh_day,
            headroom_kwh_day,
            generation_kwh_day,
            verdict,
            summary,
        })
    }

    /// A design-time buildability check over the placed machines: is there a power source for the
    /// load, does energy balance over a representative day (with the battery carrying the solar-off
    /// window), and is the wiring intact. Pure + world-free so it runs in the construction editor
    /// AND is callable by an AI before it commits a design. `sun_hours` = representative daily peak-
    /// equivalent sun (the self-sufficiency model uses ~4.5; a panel with a sourced `average_watts`
    /// ignores it). Real kWh/day, not nameplate -- this is the home-design real-world-validity
    /// guarantee. (v0.524, Stage 3 -- docs/design/home-design.md.) Since 2026-09-27 the energy
    /// balance counts only what the home makes on its own (`MachineDef::average_supply_watts`); a
    /// home that balances only by running its backstop genset every day WARNS, naming the hours and
    /// the fuel, and one that cannot balance even with the genset running all day FAILS.
    ///
    /// Run lengths (the Conduits and Data links checks) measure between the two machines' offsets
    /// as written; on a ship, where offsets are zone-local, use `buildability_report_in` with the
    /// ship's zones so a run between two zones measures in ship metres.
    pub fn buildability_report(&self, sun_hours: f32, basis: MeterBasis) -> BuildabilityReport {
        self.buildability_report_in(sun_hours, basis, None)
    }

    /// Where a machine row stands for measuring a run to another machine: its ship position
    /// (`zone_world_pos` in its zone) when the ship's zones are known, else its offset as written.
    /// Since increment 1a offsets are zone-local, so subtracting two offsets in different zones
    /// measured the home's battery to the Commons at 10 m instead of the real 75 m.
    fn run_pos(inst: &MachineInstance, zones: Option<&[ZoneRect]>) -> (f32, f32, f32) {
        match zones.and_then(|z| resolve_zone_rect(z, &inst.zone)) {
            Some(zr) => zone_world_pos(zr, inst.offset),
            None => inst.offset,
        }
    }

    /// The length of a run between machines `from` and `to` (metres, straight line, at least
    /// 1 m), measured the way the Conduits and Data links checks measure it. None when either
    /// id is not a placed machine.
    pub fn run_length(&self, from: &str, to: &str, zones: Option<&[ZoneRect]>) -> Option<f32> {
        let all = self.all_instances();
        let a = all.iter().find(|i| i.id == from)?;
        let b = all.iter().find(|i| i.id == to)?;
        let (p, q) = (Self::run_pos(a, zones), Self::run_pos(b, zones));
        Some(((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2) + (p.2 - q.2).powi(2)).sqrt().max(1.0))
    }

    /// `buildability_report` with the ship's zones (`ShipStructure::zone_rects`), so the run
    /// lengths it sizes cables and data links by are measured in ship metres (`run_pos`).
    pub fn buildability_report_in(&self, sun_hours: f32, basis: MeterBasis, zones: Option<&[ZoneRect]>) -> BuildabilityReport {
        let all = self.all_instances();
        // Sum the electrical roles across every placed machine (via its catalog def's power).
        let mut solar_peak = 0.0f32; // W at full sun
        let mut steady_watts = 0.0f32; // W a fuel-free generator makes, averaged over a day (wind)
        let mut supply_watts = 0.0f32; // W the home makes on its own, averaged over a day
        let (mut backstop_w, mut backstop_lph) = (0.0f32, 0.0f32); // gensets: W while running, L/h
        let mut consumer_watts = 0.0f32; // W draw, full
        let mut average_watts = 0.0f32; // W draw averaged over a day (2026-09-27)
        let mut battery_wh = 0.0f32; // Wh storage
        for inst in &all {
            if let Some(def) = self.catalog.get(&inst.machine) {
                supply_watts += def.average_supply_watts(sun_hours);
                // Every load the meter charges, a machine whose electrical port carries its
                // continuous-equivalent without a power role (a tower's pump, the freezer)
                // included, so the balance and the Usage meter agree (2026-09-27; this counted
                // only the Consumers, 5.7 kWh a day short of the family meter). A battery's
                // terminal is storage, not load.
                if !matches!(def.power, Some(MachinePower::Battery { .. })) {
                    consumer_watts += def.electrical_load_watts();
                }
                average_watts += def.average_load_watts(basis, is_grow_light(&inst.machine));
                match &def.power {
                    Some(MachinePower::Solar { peak_watts, .. }) => solar_peak += peak_watts,
                    Some(MachinePower::Generator { watts, fuel_lph }) => {
                        if *fuel_lph > 0.0 {
                            backstop_w += watts;
                            backstop_lph += fuel_lph;
                        } else {
                            steady_watts += watts;
                        }
                    }
                    Some(MachinePower::Battery { capacity_wh, .. }) => battery_wh += capacity_wh,
                    _ => {}
                }
            }
        }
        let sun = sun_hours.clamp(0.0, 24.0);
        let gen_daily = supply_watts * 24.0; // Wh/day the home makes on its own
        // What the loads take in a day: each one's average draw
        // (`MachineDef::average_load_watts`), not its full draw for 24 h.
        let use_daily = average_watts * 24.0; // Wh/day
        let mut checks = Vec::new();

        // 1. A power source exists for the load.
        if consumer_watts > 0.0 {
            if solar_peak <= 0.0 && steady_watts <= 0.0 && backstop_w <= 0.0 {
                checks.push(BuildabilityCheck {
                    name: "Power source".into(),
                    status: CheckStatus::Fail,
                    detail: format!("{consumer_watts:.0} W of load but no panel or generator"),
                });
            } else {
                checks.push(BuildabilityCheck {
                    name: "Power source".into(),
                    status: CheckStatus::Pass,
                    detail: format!(
                        "{solar_peak:.0} W panels + {steady_watts:.0} W steady generators (a day's average) + {backstop_w:.0} W backstop generators"
                    ),
                });
            }
        }

        // 2. Energy balances over a representative day, battery carries the solar-off window.
        if use_daily > 0.0 {
            if gen_daily + 1.0 < use_daily {
                let short_kwh = (use_daily - gen_daily) / 1000.0;
                let backstop_kwh_day = backstop_w * 24.0 / 1000.0;
                if backstop_w > 0.0 && backstop_kwh_day + 0.001 >= short_kwh {
                    // Balances only on fuel: say how much.
                    let hours = short_kwh / (backstop_w / 1000.0);
                    checks.push(BuildabilityCheck {
                        name: "Energy balance".into(),
                        status: CheckStatus::Warn,
                        detail: format!(
                            "{:.1} kWh/day made on its own < {:.1} used; the backstop generator covers the {short_kwh:.1} kWh by running {hours:.1} h a day on {:.1} L of fuel",
                            gen_daily / 1000.0,
                            use_daily / 1000.0,
                            hours * backstop_lph
                        ),
                    });
                } else {
                    let tail = if backstop_w > 0.0 {
                        format!(", and the backstop generator running all day adds only {backstop_kwh_day:.1}")
                    } else {
                        String::new()
                    };
                    checks.push(BuildabilityCheck {
                        name: "Energy balance".into(),
                        status: CheckStatus::Fail,
                        detail: format!(
                            "{:.1} kWh/day generated < {:.1} consumed{tail}",
                            gen_daily / 1000.0,
                            use_daily / 1000.0
                        ),
                    });
                }
            } else {
                // Generation covers the day; can the battery carry the load while solar is off?
                let night_h = (24.0 - sun).max(0.0);
                let night_deficit_w = (average_watts - steady_watts).max(0.0);
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
        //    needs 6 AWG). Run length is the straight line between the two machines in ship metres
        //    (`run_pos`, given the zones). A connection may pin a cable via `spec`; otherwise it's
        //    auto-sized.
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
                let len = dist(Self::run_pos(from, zones), Self::run_pos(to, zones)).max(1.0);
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
        //    pinned medium. A wireless medium is judged like a wired one, on bandwidth and range (its
        //    RF-harms-a-grow caution was removed 2026-09-27 with the crop harm). Length as the
        //    Conduits check measures it (`run_pos`).
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
                let len = dist(Self::run_pos(from, zones), Self::run_pos(to, zones)).max(1.0);
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
    /// `zones` selects the coordinate model (v0.538 box mode, generalized per-zone v0.754):
    /// - **zones = Some(rects)** (a ShipStructure multi-zone home): each instance's
    ///   `offset.0`/`offset.2` is a ZONE-LOCAL x/z (increment 1a), kept inside ITS zone's
    ///   footprint and placed at that zone's origin (`zone_world_pos`; `resolve_zone_rect` falls
    ///   back "home" -> first zone for a stale zone id) so a machine with an out-of-box offset
    ///   still lands visibly INSIDE its zone; the y base is the ZONE's floor. No instance is skipped on
    ///   a stale room id -- position no longer depends on the churning flood-fill room ids, so a
    ///   machine survives wall edits and old data still renders.
    /// - **zones = None** (a legacy AABB-room ship layout): the offset is RELATIVE to the room
    ///   center, y up from the room floor, and a machine whose room is missing is skipped --
    ///   exactly as before.
    pub fn placements(
        &self,
        rooms: &std::collections::HashMap<String, RoomGeom>,
        zones: Option<&[ZoneRect]>,
    ) -> Vec<PlacedMachine> {
        let mut out = Vec::new();
        for inst in self.all_instances() {
            let Some(def) = self.catalog.get(&inst.machine) else { continue };
            let (x, y, z, floor_y, ceiling_y) = if let Some(zones) = zones {
                // Zone-local x/z, kept inside the machine's zone, placed at that zone's origin;
                // y from that zone's floor (`zone_world_pos`).
                let Some(zr) = resolve_zone_rect(zones, &inst.zone) else { continue };
                let (x, y, z) = zone_world_pos(zr, inst.offset);
                let (_, oy, _) = zr.origin;
                (x, y, z, oy, oy + zr.size.2)
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
                stats: self.stats_for(&inst.machine),
                rotation: inst.rotation,
                model: def.model.clone(),
                // The instance's own page wins over the def's default, so two
                // wall screens of one catalog type can show different pages.
                screen: def.screen.clone().map(|mut s| {
                    if let Some(source) = &inst.screen_source {
                        s.source = source.clone();
                    }
                    s
                }),
                camera: def.camera,
                camera_px: def.camera_px,
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
                        rotation: 0.0,
                        zone: arr.zone.clone(),
                        // Array cells show their def's default page; a
                        // per-cell page needs an explicit instance.
                        screen_source: None,
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

    /// Colour (sRGB 0..1, rgba) for a connection kind: the utility-colour LEGEND of the build
    /// editor (port gizmos, the connection inspector, the meters).
    ///
    /// Since 2026-10-04 this is the ship's marking scheme (ISO 14726, data/piping/marking_schemes.ron):
    /// the content's MAIN colour, the band Simplified pipe markings draw, so the legend says what
    /// the pipes' marker bands say. The v0.622 legend it replaces was a hand-picked set (amber
    /// power, violet data, red hot water) that clashed with every published scheme
    /// (docs/reference/findings/2026-10-04-pipe-marking-standards.md). An unmarked kind is neutral
    /// grey. These are sRGB values, and several (ISO 14726's black, brown, maroon) are too dark to
    /// read on a dark background: the 3D port gizmos take them linearised and outlined
    /// (`gizmo_colours`), and the editor panels show them as an outlined swatch beside theme text,
    /// never as a text colour (2026-10-04 review).
    pub fn connection_color(kind: &str) -> [f32; 4] {
        crate::ship::pipe_marking::marking().main_colour_srgb01(kind).unwrap_or([0.6, 0.6, 0.6, 1.0])
    }

    /// The build editor's 3D port gizmo colours for a connection kind, in LINEAR light, which is
    /// what the renderer's material base colours and line colours are (2026-10-04 review: the
    /// gizmos took `connection_color`'s sRGB as if it were linear, so they were paler than the
    /// bands they stand for). `fill`, the node sphere, is the pipes' band colour. `outline`, the
    /// port's arrows and the drag line, is that colour too when it stands out against black, the
    /// darkest a room gets, at WCAG's 3:1 for graphics, else `light_outline` (a theme token,
    /// linear), so ISO 14726's black for waste or brown for fuel still shows.
    pub fn gizmo_colours(kind: &str, light_outline: [f32; 4]) -> GizmoColours {
        use crate::ship::pipe_marking::contrast_ratio;
        use crate::ship::pipe_materials::srgb_to_linear;
        let c = Self::connection_color(kind);
        let lin = |v: f32| srgb_to_linear((v * 255.0).round().clamp(0.0, 255.0) as u8);
        let fill = [lin(c[0]), lin(c[1]), lin(c[2]), 1.0];
        let outline = if contrast_ratio(fill, [0.0, 0.0, 0.0, 1.0]) >= 3.0 { fill } else { light_outline };
        GizmoColours { fill, outline }
    }

    /// Every placed machine's id -> its machine type (arrays expanded), for `line_content`.
    pub fn instance_types(&self) -> std::collections::HashMap<String, String> {
        self.all_instances().into_iter().map(|i| (i.id, i.machine)).collect()
    }

    /// What a line of connection `kind` leaving the machine `from_id` CARRIES: the content its
    /// marker bands name (2026-10-04, pipe marking review). The source machine type's
    /// `outlet_media` entry for `kind` when it has one (water leaving the purifier is
    /// `potable_water`, water leaving an air handler `condensate`), else `kind` itself, whose
    /// scheme row marks only its group. `types` is `instance_types()`; a conduit-node end
    /// ("node:...") or an unknown id carries its kind. Derived from the machine the line leaves,
    /// never from anything typed on the line, so the markers stay honest by construction.
    pub fn line_content<'a>(&'a self, types: &std::collections::HashMap<String, String>, from_id: &str, kind: &'a str) -> &'a str {
        types
            .get(from_id)
            .and_then(|t| self.catalog.get(t))
            .and_then(|d| d.outlet_media.get(kind))
            .map(String::as_str)
            .unwrap_or(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build editor's utility legend (`connection_color`: the port gizmos, the connection
    /// inspector, the meters) is the ship's marking scheme, so it says what the pipes' marker
    /// bands say (2026-10-04). Seen red with the v0.622 legend still in place: "power's legend is
    /// the scheme's simplified band: [0.95, 0.75, 0.15, 1.0]".
    #[test]
    fn connection_colour_is_the_ship_schemes_main_colour() {
        let reg = crate::ship::pipe_marking::marking();
        let power = MachineHome::connection_color("power");
        assert_eq!(power, reg.main_colour_srgb01("power").unwrap(), "power's legend is the scheme's simplified band: {power:?}");
        for kind in ["water", "potable_water", "condensate", "hot_water", "air", "gas", "fuel", "data", "nutrient", "waste", "greywater"] {
            assert_eq!(MachineHome::connection_color(kind), reg.main_colour_srgb01(kind).unwrap(), "{kind}");
        }
        assert_eq!(MachineHome::connection_color("no_such_utility"), [0.6, 0.6, 0.6, 1.0], "unknown stays neutral grey");
        assert_eq!(MachineHome::connection_color("food"), [0.6, 0.6, 0.6, 1.0], "an unmarked content is neutral grey");
    }

    /// The 3D port gizmos draw the pipes' band colour in LINEAR light, as the bands do (the
    /// renderer's material and line colours are linear), and their arrows stay visible however
    /// dark that colour is: at least WCAG's 3:1 for graphics against black, the darkest a room
    /// gets (2026-10-04 review: ISO 14726's black for waste, compost and grey water went in as
    /// sRGB and drew a near-black node with near-black arrows).
    ///
    /// Seen red with the gizmos taking `connection_color` as it is: "`water`'s port node is its
    /// band colour in linear light: [0.09411765, 0.34509805, 0.72156864, 1.0], the band is
    /// [0.009134057, 0.09758736, 0.47932023, 1.0]".
    #[test]
    fn port_gizmos_draw_the_band_colour_in_linear_light_and_stay_visible() {
        use crate::ship::pipe_marking::{contrast_ratio, marking};
        let ship = marking().default_scheme().expect("the ship's scheme");
        let light = [0.45, 0.45, 0.49, 1.0]; // the theme's secondary text, linearised
        let black = [0.0, 0.0, 0.0, 1.0];
        let mut dark = 0;
        for row in ship.contents.iter().filter(|r| !r.unmarked) {
            let g = MachineHome::gizmo_colours(&row.content, light);
            let band = ship.colour(&row.main).expect("its main colour").linear_rgba();
            assert_eq!(g.fill, band, "`{}`'s port node is its band colour in linear light: {:?}, the band is {:?}", row.content, g.fill, band);
            let c = contrast_ratio(g.outline, black);
            assert!(c >= 3.0, "`{}`'s port arrows show against a dark room: {c:.2}:1 ({:?})", row.content, g.outline);
            if contrast_ratio(band, black) < 3.0 {
                dark += 1;
                assert_eq!(g.outline, light, "`{}`'s band is too dark to outline itself: the light outline", row.content);
            } else {
                assert_eq!(g.outline, band, "`{}` outlines in its own band colour", row.content);
            }
        }
        assert!(dark >= 3, "sanity: ISO 14726's black and brown rows need the outline ({dark})");
        // An unmarked or unknown content is neutral grey, linearised, and visible.
        let food = MachineHome::gizmo_colours("food", light);
        assert!(contrast_ratio(food.outline, black) >= 3.0 && food.fill == food.outline, "{food:?}");
    }

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

    /// EVERY machine stands in the room it claims (2026-09-19, the home redesign).
    ///
    /// In zone mode an instance's `offset` x/z is an ABSOLUTE world position that the
    /// renderer merely CLAMPS into its zone, so a machine with the wrong coordinates does
    /// not fail loudly: it slides to the edge of the acre and sits there. `room` now
    /// carries the zone id of the room it belongs to, which makes the intent checkable, and
    /// this is the check. It is what turns "I measured every placement carefully" into
    /// "the build fails if I did not".
    ///
    /// PROVEN RED: move any instance's x or z outside its room's rect and this names the
    /// machine, where it is, and the rect it should be in.
    #[test]
    fn every_placed_machine_stands_inside_the_room_it_names() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&root.join("data"), None)
            .expect("the shipped ship assembles");
        let home_zone = &ship.zones[ship.home_zone_index()];
        // Only the sub-zones INSIDE the acre are rooms (the mothership's macro districts moved
        // to ship level in increment 1a; the filter stays as the guard). Machine offsets and room
        // rects are both home-local, so they compare directly wherever the plot is.
        let rooms: std::collections::HashMap<&str, &crate::ship::home_structure::Zone> = home_zone
            .body
            .zones
            .iter()
            .filter(|z| {
                z.origin.0 >= 0.0
                    && z.origin.2 >= 0.0
                    && z.origin.0 + z.size.0 <= home_zone.body.width
                    && z.origin.2 + z.size.2 <= home_zone.body.depth
            })
            .map(|z| (z.id.as_str(), z))
            .collect();
        assert!(rooms.len() >= 20, "the acre is partitioned into rooms, got {}", rooms.len());

        for file in ["home.ron", "home_solo.ron"] {
            let home = MachineHome::load(&root.join("data").join("machines").join(file))
                .unwrap_or_else(|| panic!("{file} parses"));
            let mut checked = 0usize;
            for inst in home.all_instances() {
                if inst.zone != "home" {
                    continue; // another zone's machine (the Commons), not this acre's
                }
                let Some(z) = rooms.get(inst.room.as_str()) else {
                    panic!(
                        "{file}: machine '{}' names room '{}', which is not a room zone in data/homes/shipped/homestead.ron",
                        inst.id, inst.room
                    );
                };
                let (x, zz) = (inst.offset.0, inst.offset.2);
                assert!(
                    x >= z.origin.0
                        && x <= z.origin.0 + z.size.0
                        && zz >= z.origin.2
                        && zz <= z.origin.2 + z.size.2,
                    "{file}: machine '{}' ({}) at ({x}, {zz}) is outside '{}' [{}..{}] x [{}..{}]",
                    inst.id,
                    inst.machine,
                    inst.room,
                    z.origin.0,
                    z.origin.0 + z.size.0,
                    z.origin.2,
                    z.origin.2 + z.size.2
                );
                checked += 1;
            }
            assert!(checked > 30, "{file}: expected a furnished home, only checked {checked} machines");
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
        let tmp = std::env::temp_dir().join(format!("humanity_home_roundtrip_{}.ron", std::process::id()));
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
            rotation: 0.0,
            zone: "home".to_string(),
            screen_source: None,
        });
        let tmp = std::env::temp_dir().join(format!("humanity_home_add_{}.ron", std::process::id()));
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
            outlet_media: BTreeMap::new(),
            auto_recipe: None,
            irrigates: false,
            auto_keep: None,
            lights_crops: false,
            pollinates_crops: false,
            ventilation_m3_h: 0.0,
            co2_setpoint_ppm: 0.0,
            humidifies_l_h: 0.0,
            dehumidifies_m3_h: 0.0,
            scrubs_co2_kg_day: 0.0,
            level_gauge: false,
            container_type: None,
            provides: None,
            model: None,
            screen: None,
            camera: None,
            camera_px: CAMERA_PX_DEFAULT,
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
            ship_machines: None,
            ship_part: Default::default(),
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
                zone: "home".to_string(),
            }],
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
            rotation: 0.0,
            zone: "home".to_string(),
            screen_source: None,
        };
        let conn = |from: &str, to: &str| MachineConnection {
            from: from.to_string(),
            to: to.to_string(),
            kind: "water".to_string(),
            spec: None,
        };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: vec![conn("a", "b"), conn("b", "c"), conn("c", "a")],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
            rotation: 0.0,
            zone: "home".to_string(),
            screen_source: None,
        };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
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
                zone: "home".to_string(),
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
            grown: Default::default(),
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
            rotation: 0.0,
            zone: "home".to_string(),
            screen_source: None,
        };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("a"), inst("b")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![MachineInstance { id: "solo".into(), machine: "box".into(), room: "garden".into(), offset: (1.0, 0.0, 2.0), rotation: 0.0, zone: "home".into(), screen_source: None }],
            arrays: vec![MachineArray {
                machine: "box".to_string(),
                room: "garden".to_string(),
                origin: (10.0, 0.0, 20.0),
                rows: 2,
                cols: 3, // 6 cells: tray_0..tray_5
                spacing: (1.5, 2.0),
                id_prefix: "tray".to_string(),
                zone: "home".to_string(),
            }],
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
        let inst = |id: &str| MachineInstance { id: id.into(), machine: "box".into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: Vec::new(),
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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

    /// v0.633: a machine's yaw rotation round-trips through RON AND is carried into its placement (so the
    /// renderer can orient the mesh). A new instance with no `rotation` in the RON defaults to 0.
    #[test]
    fn machine_rotation_round_trips_and_placements_carry_it() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![MachineInstance { id: "m1".into(), machine: "box".into(), room: "g".into(), offset: (1.0, 0.0, 2.0), rotation: 90.0, zone: "home".into(), screen_source: None }],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        // placements (box mode) carry the yaw through to the renderer.
        let placed = home.placements(&std::collections::HashMap::new(), Some(&one_zone(20.0, 20.0, 4.0)));
        let m = placed.iter().find(|p| p.id == "m1").expect("m1 placed");
        assert_eq!(m.rotation, 90.0, "placement carries the instance rotation");
        // RON round-trip preserves it.
        let ron = ron::ser::to_string(&home).expect("serialize");
        let back: MachineHome = ron::from_str(&ron).expect("deserialize");
        assert_eq!(back.instances[0].rotation, 90.0, "rotation survives the round-trip");
        // An array-expanded cell defaults to 0 yaw.
        home.arrays.push(MachineArray { machine: "box".into(), room: "g".into(), origin: (5.0, 0.0, 5.0), rows: 1, cols: 1, spacing: (1.0, 1.0), id_prefix: "cell".into(), zone: "home".into() });
        let cell = home.all_instances().into_iter().find(|i| i.id == "cell_0").expect("cell exists");
        assert_eq!(cell.rotation, 0.0, "an array cell defaults to 0 yaw");
    }

    /// A machine def carrying a specific electrical role, for buildability tests.
    fn def_with_power(power: Option<MachinePower>) -> MachineDef {
        MachineDef { power, ..test_def("box") }
    }

    /// v0.524 Stage 3: a load with no panel/generator fails the "Power source" check.
    #[test]
    fn buildability_flags_load_without_a_source() {
        let mut catalog = BTreeMap::new();
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![MachineInstance { id: "l1".into(), machine: "load".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None }],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let report = home.buildability_report(4.5, MeterBasis::default());
        assert_eq!(report.worst(), CheckStatus::Fail);
        assert!(report.checks.iter().any(|c| c.name == "Power source" && c.status == CheckStatus::Fail));
    }

    /// v0.630 grid S2: utility_meters reports per-utility daily generation vs demand + a self-sufficiency
    /// fraction, non-punitively. A 1000 W panel + a 100 W load: 4.5 kWh/day made vs 2.4 used -> self-suff.
    #[test]
    fn utility_meters_report_generation_demand_and_self_sufficiency() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("p1", "panel"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let meters = home.utility_meters(4.5, MeterBasis::default());
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

    /// The static meters charge what the machines really draw (2026-09-27): a ship life support
    /// machine nothing in the Station-supplied mode and its measured `average_watts` in the
    /// Realistic one, a work station its idle draw (its working draw named in the summary, not
    /// charged for the day), a grow light its timer's hours, anything else its full draw. And on
    /// the shipped family home, the two modes differ by exactly its seven air handlers' measured
    /// average. Seen red by charging every consumer its full electrical load for 24 h again (the
    /// old meter: an air handler read 7.8 kWh a day in either mode, the stove 28.8).
    #[test]
    fn the_static_meters_charge_what_the_machines_draw() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        let mut handler = def_with_power(Some(MachinePower::Consumer { watts: 325.0, priority: 2, idle_watts: None, average_watts: Some(200.0) }));
        handler.dehumidifies_m3_h = 1842.0;
        catalog.insert("air_handler".to_string(), handler);
        catalog.insert("stove".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 1200.0, priority: 2, idle_watts: Some(0.0), average_watts: None })));
        catalog.insert("grow_light".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 5, idle_watts: None, average_watts: None })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("p1", "panel"), inst("a1", "air_handler"), inst("s1", "stove"), inst("g1", "grow_light"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
        };
        let power = |basis: MeterBasis| home.utility_meters(4.5, basis).into_iter().find(|m| m.utility == "power").unwrap();
        let (station, realistic) = (power(MeterBasis { life_support_on_grid: false }), power(MeterBasis { life_support_on_grid: true }));
        // Station-supplied: the handler 0, the idle stove 0, the light 100 W x 6 h, the load 100 W x 24 h.
        assert!((station.demand - 3.0).abs() < 1e-3, "station {}", station.demand);
        // Realistic: plus the handler's 200 W average for the day.
        assert!((realistic.demand - 7.8).abs() < 1e-3, "realistic {}", realistic.demand);
        assert!(station.summary.contains("1.2 kW more while a craft runs"), "{}", station.summary);
        let report = home.buildability_report(4.5, MeterBasis { life_support_on_grid: true });
        assert!(report.checks.iter().any(|c| c.name == "Energy balance" && c.detail.contains("< 7.8 consumed")), "{:?}", report.checks);

        // The shipped family home: the modes differ by its air handlers' measured average.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        let family = MachineHome::load(&path).expect("home.ron parses");
        let kwh = |basis: MeterBasis| family.utility_meters(4.5, basis).into_iter().find(|m| m.utility == "power").unwrap().demand;
        let (s, r) = (kwh(MeterBasis { life_support_on_grid: false }), kwh(MeterBasis { life_support_on_grid: true }));
        let Some(MachinePower::Consumer { average_watts: Some(avg), .. }) = family.catalog["air_handler"].power else { panic!("an average") };
        let handlers = family.all_instances().iter().filter(|i| i.machine == "air_handler").count() as f32;
        let old: f32 = family
            .all_instances()
            .iter()
            .filter_map(|i| family.catalog.get(&i.machine))
            .filter(|d| !matches!(d.power, Some(MachinePower::Battery { .. })))
            .map(|d| d.electrical_load_watts())
            .sum::<f32>()
            * 24.0
            / 1000.0;
        println!("family home: {s:.1} kWh/day Station-supplied, {r:.1} Realistic; the old nameplate meter {old:.1}");
        // What the Station-supplied figure is made of, largest first.
        let mut by_type: BTreeMap<String, (usize, f32)> = BTreeMap::new();
        for i in family.all_instances() {
            if let Some(d) = family.catalog.get(&i.machine) {
                let e = by_type.entry(i.machine.clone()).or_insert((0, 0.0));
                e.0 += 1;
                e.1 += d.average_load_watts(MeterBasis::default(), is_grow_light(&i.machine)) * 24.0 / 1000.0;
            }
        }
        let mut rows: Vec<_> = by_type.into_iter().filter(|(_, (_, k))| *k > 0.0).collect();
        rows.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
        for (id, (n, k)) in rows.iter().take(12) {
            println!("  {id}: {n} placed, {k:.1} kWh/day");
        }
        assert!((r - s - handlers * avg * 24.0 / 1000.0).abs() < 0.01, "{r} - {s} is the {handlers} handlers' {avg} W");
    }

    /// The household machines charge the meter their real daily energy (2026-09-27), and each
    /// home's Energy loop says what the meter says. Before, the family water heater charged its
    /// 2 kW element for 24 hours (48.0 kWh a day), the washer its 500 W (12.0) and the 33 towers
    /// their 15 W ports (11.9), and the freezer charged nothing. Now: water heating, the washer's
    /// motor and the freezer are EIA's 2020 RECS averages for a household of the home's size
    /// (Tables CE5.3a and CE5.3b, kWh a year per household using the end use), and a tower's
    /// pump is its maker's rating on the maker's indoor timer
    /// (Tower Garden: 48 W or 35 W, 5 minutes on in every 50). The sources are quoted beside each
    /// machine in data/machines/*.ron. The electrical sim has no thermostat or wash-cycle model,
    /// so the water heater's and washer's runtime draw (`watts`) must be the same average, or
    /// the game drains the batteries at the nameplate while the meter says otherwise.
    ///
    /// Also since 2026-09-27: each fish tank's air pump (it had a 50 W stat and no power) is a
    /// HIBLOW HP-20, 17 W for 20 L of air a minute, inside FAO 589's 5 to 8 L a minute for each
    /// cubic metre of the tank's water; the Energy loop's supply is the meter's generation (what
    /// the home makes on its own, `the_power_sources_make_their_sourced_daily_energy`), and its
    /// `closes` is judged against that; and data/towers/aeroponic_configs.ron's pump timers say
    /// the indoor setting the pumps are charged on (they suggested "15 min on / 15 off").
    ///
    /// Seen red: home.ron's water heater `average_watts` removed (the meter read its 2 kW port,
    /// 48.0 kWh a day); the variety tower's port put back to 15 W (the tower check named it); the
    /// family Energy loop's Station-supplied figure left at the old 15.8 (the loop check named
    /// it against the meter's figure); home_solo.ron's fish tank without its power (charged 0 W);
    /// the family loop's supply left at 15.1 (against the meter's 10.98); the variety tower's
    /// timer note put back to "15 min on / 15 off"; and the Buildability energy balance counting
    /// only the machines with a power role again (it charged the family 18.3 kWh a day against
    /// the Usage meter's 24.0, leaving out the towers' pumps and the freezer).
    #[test]
    fn the_household_machines_charge_their_sourced_daily_energy() {
        // EIA RECS 2020 Table CE5.3a, kWh a year per household using the end use.
        const WATER_HEATING_KWH_YR: [(usize, f32); 2] = [(1, 1427.0), (3, 3482.0)];
        const CLOTHES_WASHER_KWH_YR: [(usize, f32); 2] = [(1, 46.0), (3, 77.0)];
        // And Table CE5.3b, separate freezers (the freezer had no power data at all).
        const FREEZER_KWH_YR: [(usize, f32); 2] = [(1, 539.0), (3, 559.0)];
        // Tower Garden pumps (W) and the maker's indoor timer, "5 min on, 45 min off".
        const PUMPS: [(&str, f32); 2] = [("aeroponic_tower_nutrition", 48.0), ("aeroponic_tower_apothecary", 35.0)];
        const INDOOR_DUTY: f32 = 5.0 / 50.0;
        // The fish tank's air pump: HIBLOW HP-20, "Power Consumption W 17", 20 L/min at 9.8 kPa;
        // FAO 589: "5–8 litres of air per minute for each cubic metre of water".
        const HP20_W: f32 = 17.0;
        const HP20_L_MIN: f32 = 20.0;
        const FAO_L_MIN_PER_M3: (f32, f32) = (5.0, 8.0);
        let per_year = |kwh: f32| kwh * 1000.0 / 8760.0;
        let recs = |table: &[(usize, f32)], people: usize| table.iter().find(|(n, _)| *n == people).map(|(_, k)| per_year(*k)).unwrap();
        // The figure the loop text states just before `tail` ("~21.2 kWh/day Station-supplied").
        let stated = |text: &str, tail: &str| -> f32 {
            let end = text.find(tail).unwrap_or_else(|| panic!("the Energy loop's demand has no `{tail}`: {text}"));
            let start = text[..end].rfind('~').expect("a ~figure before it");
            text[start + 1..end].trim().parse::<f32>().unwrap_or_else(|e| panic!("`{}`: {e}", &text[start + 1..end]))
        };
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let root = data.join("machines");
        for (file, people) in [("home.ron", 3usize), ("home_solo.ron", 1usize)] {
            let home = MachineHome::load(&root.join(file)).unwrap_or_else(|| panic!("{file} parses"));
            let charged = |id: &str| home.catalog[id].average_load_watts(MeterBasis::default(), false);
            let runtime = |id: &str| match home.catalog[id].power {
                Some(MachinePower::Consumer { watts, .. }) => watts,
                _ => panic!("{file}: {id} is a Consumer"),
            };
            for (id, want) in [("water_heater", recs(&WATER_HEATING_KWH_YR, people)), ("washer", recs(&CLOTHES_WASHER_KWH_YR, people))] {
                assert!((charged(id) - want).abs() < 0.1, "{file}: the meter charges {id} {} W, RECS gives {want:.2} W for {people}", charged(id));
                assert!((runtime(id) - want).abs() < 0.1, "{file}: {id} draws {} W in play, its daily average is {want:.2} W", runtime(id));
            }
            let freezer = recs(&FREEZER_KWH_YR, people);
            assert!((charged("freezer") - freezer).abs() < 0.1, "{file}: the meter charges the freezer {} W, RECS gives {freezer:.2} W for {people}", charged("freezer"));
            for (id, pump) in PUMPS {
                assert!((charged(id) - pump * INDOOR_DUTY).abs() < 1e-3, "{file}: the meter charges {id} {} W; its {pump} W pump on the indoor timer averages {}", charged(id), pump * INDOOR_DUTY);
            }
            // The fish tank's air pump, all day, sized to FAO's rate for the tank's water.
            let tank = &home.catalog["aquaponic_tank"];
            let m3 = tank.size.0 * tank.size.1 * tank.size.2;
            assert!(
                HP20_L_MIN >= FAO_L_MIN_PER_M3.0 * m3 && HP20_L_MIN <= FAO_L_MIN_PER_M3.1 * m3 + 0.5,
                "{file}: {HP20_L_MIN} L/min of air for a {m3} m3 tank, FAO asks {} to {}",
                FAO_L_MIN_PER_M3.0 * m3,
                FAO_L_MIN_PER_M3.1 * m3
            );
            assert!((charged("aquaponic_tank") - HP20_W).abs() < 1e-3, "{file}: the meter charges the fish tank {} W; its air pump draws {HP20_W} W all day", charged("aquaponic_tank"));
            assert!((runtime("aquaponic_tank") - HP20_W).abs() < 1e-3, "{file}: the fish tank's air pump draws {} W in play", runtime("aquaponic_tank"));

            // The Energy loop states the meter's figures, and whether the demand closes against
            // what the home makes on its own.
            let power = |basis: MeterBasis| home.utility_meters(4.5, basis).into_iter().find(|m| m.utility == "power").expect("a power meter");
            let (station, realistic) = (power(MeterBasis { life_support_on_grid: false }).demand, power(MeterBasis { life_support_on_grid: true }).demand);
            let made = power(MeterBasis::default()).generation;
            let energy = home.loops.iter().find(|l| l.name == "Energy").expect("an Energy loop");
            let (said_station, said_realistic) = (stated(&energy.demand, " kWh/day Station-supplied"), stated(&energy.demand, " kWh/day in the Realistic mode"));
            let supply = stated(&energy.supply, " kWh/day:");
            println!("{file}: meter {station:.2} kWh/day Station-supplied, {realistic:.2} Realistic, made {made:.2}; the loop says {said_station} and {said_realistic} against {supply}");
            if let Some(m) = home.utility_meters(4.5, MeterBasis::default()).into_iter().find(|m| m.utility == "power") {
                println!("  summary: {}", m.summary);
            }
            if let Some(m) = home.utility_meters(4.5, MeterBasis { life_support_on_grid: true }).into_iter().find(|m| m.utility == "power") {
                println!("  Realistic summary: {}", m.summary);
            }
            for c in home.buildability_report(4.5, MeterBasis::default()).checks.iter().chain(home.buildability_report(4.5, MeterBasis { life_support_on_grid: true }).checks.iter()) {
                if c.name == "Energy balance" || c.name == "Power source" {
                    println!("  {}: {:?} {}", c.name, c.status, c.detail);
                }
            }
            let mut by_type: BTreeMap<String, (usize, f32)> = BTreeMap::new();
            for i in home.all_instances() {
                if let Some(d) = home.catalog.get(&i.machine) {
                    let e = by_type.entry(i.machine.clone()).or_insert((0, 0.0));
                    e.0 += 1;
                    e.1 += d.average_load_watts(MeterBasis::default(), is_grow_light(&i.machine)) * 24.0 / 1000.0;
                }
            }
            let mut rows: Vec<_> = by_type.into_iter().filter(|(_, (_, k))| *k > 0.0).collect();
            rows.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
            for (id, (n, k)) in &rows {
                println!("  {id}: {n} placed, {k:.3} kWh/day");
            }
            assert!((said_station - station).abs() <= 0.05, "{file}: the Energy loop says ~{said_station} kWh/day Station-supplied, the meter {station:.2}");
            assert!((said_realistic - realistic).abs() <= 0.05, "{file}: the Energy loop says ~{said_realistic} kWh/day Realistic, the meter {realistic:.2}");
            assert!((supply - made).abs() <= 0.05, "{file}: the Energy loop says the home makes ~{supply} kWh/day, the meter {made:.2}");
            assert_eq!(energy.closes, station <= made && realistic <= made, "{file}: the Energy loop's `closes` against the {made:.2} kWh/day the home makes");
            // The Buildability panel's energy balance charges what the Usage meter charges.
            let b = home.buildability_report(4.5, MeterBasis::default()).checks.into_iter().find(|c| c.name == "Energy balance").expect("an energy balance");
            let used = format!("{station:.1}");
            assert!(
                [format!("< {used} used"), format!("< {used} consumed"), format!("vs {used} used")].iter().any(|s| b.detail.contains(s.as_str())),
                "{file}: the energy balance says `{}`, the Usage meter {used} kWh/day",
                b.detail
            );
        }

        // The towers' pump timers in the tower configs say the setting the pumps are charged on.
        let configs = std::fs::read_to_string(data.join("towers").join("aeroponic_configs.ron")).expect("the tower configs");
        let timers: Vec<&str> = configs.lines().filter(|l| l.contains("\"Repeat cycle timer\"")).collect();
        assert!(!timers.is_empty(), "the tower configs list a pump timer");
        for line in timers {
            let minutes = |tail: &str| -> f32 {
                let end = line.find(tail).unwrap_or_else(|| panic!("the timer note has no `{tail}`: {line}"));
                let start = line[..end].rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
                line[start..end].parse().unwrap_or_else(|e| panic!("`{}`: {e}", &line[start..end]))
            };
            let (on, off) = (minutes(" min on"), minutes(" min off"));
            assert!((on / (on + off) - INDOOR_DUTY).abs() < 1e-6, "a tower timer says {on} on and {off} off, the pumps are charged on {INDOOR_DUTY} of the time: {line}");
        }
    }

    /// What a Primus AIR 40 averages in a Rayleigh wind of `mean` m/s (2026-09-27): its power a
    /// cube law from its 3.13 m/s start-up speed, capped at its rated 160 W, fitted to its
    /// manual's "40 kWh/month @ 12 mph (5.5 m/s)" (the fit, `k`, is found by bisection), and
    /// averaged over the Rayleigh distribution AWEA 9.1-2009 rates small turbines by.
    fn air40_average_watts(mean: f64) -> f64 {
        const START_M_S: f64 = 3.13;
        const RATED_W: f64 = 160.0;
        let average = |u: f64, k: f64| -> f64 {
            let dv = 0.002;
            let mut sum = 0.0;
            let mut v = dv / 2.0;
            while v < 40.0 {
                let f = std::f64::consts::PI * v / (2.0 * u * u) * (-std::f64::consts::PI * v * v / (4.0 * u * u)).exp();
                if v >= START_M_S {
                    sum += f * (k * v * v * v).min(RATED_W) * dv;
                }
                v += dv;
            }
            sum
        };
        let target = 40_000.0 / (365.25 / 12.0 * 24.0); // 40 kWh a month, as watts
        let (mut lo, mut hi) = (0.0_f64, 2.0_f64);
        for _ in 0..50 {
            let mid = (lo + hi) / 2.0;
            if average(5.5, mid) < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        average(mean, (lo + hi) / 2.0)
    }

    /// The meter's supply is what each home makes on its own at its site (2026-09-27; it counted
    /// the backstop genset's 2 kW and the wind turbine's 150 W for 24 hours and each panel's 400 W
    /// for 4.5 sun-hours with nothing lost, so the family meter said it made 69.6 kWh a day,
    /// "fully self-sufficient", against its own Energy loop's 15.1). Now: a panel's
    /// `average_watts` is NREL PVWatts' 440.68 kWh a year at the site less Tesla's 90% battery
    /// round trip; the turbine's day-average `watts` is the AIR 40 in the Bremerton airport's
    /// 5.3 mph mean wind (recomputed here from the maker's figures, `air40_average_watts`), its
    /// 160 W nameplate on its port; and both homes carry the same backstop genset, a Honda
    /// EU2200i class set (1.8 kVA rated, a 3.6 L tank lasting 3.2 h at that load), which the meter
    /// shows apart and never counts as supply.
    ///
    /// Seen red three ways: home_solo.ron's generator left without its power role (the solo home
    /// had no backstop); the family wind turbine's `watts` put back to 150 (the recomputed
    /// average named it); and the backstop counted as supply again (`average_supply_watts` for a
    /// fueled genset: the genset check named it, and the family meter made 54.2 kWh a day).
    #[test]
    fn the_power_sources_make_their_sourced_daily_energy() {
        const PVWATTS_AC_KWH_YR: f32 = 440.68; // PVWatts 8.5.0, 0.4 kW at Silverdale, WA
        const BATTERY_ROUND_TRIP: f32 = 0.90; // Tesla Powerwall 2 datasheet
        const BREMERTON_MEAN_MPH: f64 = 5.3; // WRCC, KPWT 1996-2006
        const GENSET_W: f32 = 1800.0; // Honda EU2200i "Rated output" 1.8 kVA
        const GENSET_L_PER_H: f32 = 3.6 / 3.2; // its 3.6 L tank, "3.2hr @ rated load"
        let panel_w = PVWATTS_AC_KWH_YR * BATTERY_ROUND_TRIP * 1000.0 / 8760.0;
        let wind_w = air40_average_watts(BREMERTON_MEAN_MPH * 0.44704) as f32;
        println!("a panel averages {panel_w:.2} W, the turbine {wind_w:.2} W ({:.1}% of 160 W)", wind_w / 160.0 * 100.0);
        assert!((4.2..4.7).contains(&wind_w), "the AIR 40 in a 2.37 m/s wind averages {wind_w} W");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        for file in ["home.ron", "home_solo.ron"] {
            let home = MachineHome::load(&root.join(file)).unwrap_or_else(|| panic!("{file} parses"));
            let made = |id: &str| home.catalog[id].average_supply_watts(4.5);
            assert!((made("solar_panel") - panel_w).abs() < 0.05, "{file}: a panel makes {} W averaged, PVWatts less the batteries gives {panel_w:.2}", made("solar_panel"));
            assert!((made("wind_turbine_small") - wind_w).abs() < 0.1, "{file}: the turbine makes {} W averaged, the AIR 40 in the site's wind {wind_w:.2}", made("wind_turbine_small"));
            assert!((home.catalog["wind_turbine_small"].electrical_supply_watts() - 160.0).abs() < 1e-3, "{file}: the turbine's port carries its 160 W nameplate");
            assert_eq!(made("generator_portable"), 0.0, "{file}: a backstop genset makes nothing on its own");
            let (w, lph) = home.catalog["generator_portable"].backstop_watts();
            assert!((w - GENSET_W).abs() < 1e-3 && (lph - GENSET_L_PER_H).abs() < 1e-3, "{file}: the backstop runs at {w} W on {lph} L an hour, the EU2200i at {GENSET_W} on {GENSET_L_PER_H}");

            // The meter makes exactly the placed sources' averages, and shows the backstop apart.
            let all = home.all_instances();
            let count = |id: &str| all.iter().filter(|i| i.machine == id).count() as f32;
            let want = (count("solar_panel") * panel_w + count("wind_turbine_small") * wind_w) * 24.0 / 1000.0;
            let m = home.utility_meters(4.5, MeterBasis::default()).into_iter().find(|m| m.utility == "power").expect("a power meter");
            println!("{file}: makes {:.2} kWh/day, backstop {} W on {} L/h; {}", m.generation, m.backstop_watts, m.backstop_fuel_lph, m.summary);
            assert!((m.generation - want).abs() < 0.02, "{file}: the meter says the home makes {} kWh/day, its panels and turbine {want:.2}", m.generation);
            assert!((m.backstop_watts - count("generator_portable") * GENSET_W).abs() < 1e-3, "{file}: the meter's backstop is {} W", m.backstop_watts);
            assert!(m.summary.contains("not counted: a backstop generator"), "{file}: {}", m.summary);
        }
    }

    /// A backstop genset is shown apart from what the home makes (2026-09-27): the power meter,
    /// the grow-light meter and the energy balance count a panel's sourced average and a steady
    /// generator's day average, never the genset; the balance WARNS when the home covers its day
    /// only by running the genset (naming the hours and the fuel) and FAILS when even a whole
    /// day of it falls short. Seen red by counting the genset's `watts` for 24 hours again (the
    /// old meter: this home "made" 44.4 kWh a day).
    #[test]
    fn the_meter_shows_a_backstop_apart_from_what_the_home_makes() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 400.0, average_watts: Some(45.3) })));
        catalog.insert("wind".to_string(), def_with_power(Some(MachinePower::Generator { watts: 4.4, fuel_lph: 0.0 })));
        catalog.insert("genset".to_string(), def_with_power(Some(MachinePower::Generator { watts: 1800.0, fuel_lph: 1.125 })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 4000.0, max_charge_w: 2000.0, max_discharge_w: 2000.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 1000.0, priority: 1, idle_watts: None, average_watts: None })));
        catalog.insert("big_load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 3000.0, priority: 1, idle_watts: None, average_watts: None })));
        catalog.insert("grow_light".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 5, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = |loads: Vec<MachineInstance>| {
            let mut instances = vec![inst("p1", "panel"), inst("w1", "wind"), inst("g1", "genset"), inst("b1", "batt")];
            instances.extend(loads);
            MachineHome {
                ship_machines: None,
                ship_part: Default::default(),
                catalog: catalog.clone(),
                instances,
                arrays: Vec::new(),
                connections: Vec::new(),
                loops: Vec::new(),
                conduit_nodes: Vec::new(),
                conduit_edges: Vec::new(),
                grown: Default::default(),
            }
        };
        // One 1 kW load: 24 kWh a day against the (45.3 + 4.4) W x 24 h = 1.19 the home makes.
        let small = home(vec![inst("l1", "load"), inst("gl", "grow_light")]);
        let m = small.utility_meters(4.5, MeterBasis::default()).into_iter().find(|m| m.utility == "power").unwrap();
        assert!((m.generation - 1.1928).abs() < 1e-3, "the home makes {} kWh/day, not its genset", m.generation);
        assert!((m.backstop_watts - 1800.0).abs() < 1e-3 && (m.backstop_fuel_lph - 1.125).abs() < 1e-6, "{m:?}");
        // 24.6 - 1.19 = 23.41 kWh short: 13.0 h of the genset on 14.6 L.
        assert!(m.summary.contains("would run 13.0 h a day on 14.6 L of fuel"), "{}", m.summary);
        let gl = small.grow_light_report(4.5, MeterBasis::default()).expect("a grow light");
        assert!((gl.generation_kwh_day - 1.1928).abs() < 1e-3, "the grow-light meter's generation is {}", gl.generation_kwh_day);
        let balance = |h: &MachineHome| h.buildability_report(4.5, MeterBasis::default()).checks.into_iter().find(|c| c.name == "Energy balance").expect("an energy balance");
        let b = balance(&small);
        assert_eq!(b.status, CheckStatus::Warn, "{}", b.detail);
        assert!(b.detail.contains("backstop generator covers") && b.detail.contains("13.0 h a day"), "{}", b.detail);
        // A 3 kW load as well: 96.6 kWh a day, more than the genset's 43.2 on top of 1.19.
        let big = home(vec![inst("l1", "load"), inst("l2", "big_load"), inst("gl", "grow_light")]);
        let b = balance(&big);
        assert_eq!(b.status, CheckStatus::Fail, "{}", b.detail);
        assert!(b.detail.contains("running all day adds only 43.2"), "{}", b.detail);
        let m = big.utility_meters(4.5, MeterBasis::default()).into_iter().find(|m| m.utility == "power").unwrap();
        assert!(m.summary.contains("even running all day (27.0 L of fuel) would make only 43.2"), "{}", m.summary);
    }

    /// Each home's hot water adds up (2026-09-27): every fixture's hot feed is the measured hot
    /// share of that fixture's water (the Water Research Foundation's Residential End Uses of
    /// Water, 94 homes, as LBNL tables it for the EPA: showers 66.2%, faucets 57.0%, clothes
    /// washers 20.0%), the fixtures' hot feeds add up to exactly what the water heater makes,
    /// the heater takes in as much cold as it gives out hot, the washer has a hot and a cold
    /// feed and a hose from the heater, and the household's taps and fixtures draw
    /// data/home_outline.json's one-person fixture list (85 L a day) for each resident. Before,
    /// the family's hot ports added to 403 L a day against its heater's 432 and a 240 L
    /// household (whose tap drew 245 on top of fixtures drawing 1,267), and the washer had only
    /// a cold feed. Seen red three ways: the family shower's hot feed put back to 0.20 L/min;
    /// home_solo.ron's washer without its hot fill; and the family household tap put back to
    /// 0.17 L/min.
    #[test]
    fn the_household_hot_water_is_each_fixtures_measured_share() {
        const HOT_SHARE: [(&str, f32); 4] = [("shower", 0.662), ("kitchen_sink", 0.570), ("bath_sink", 0.570), ("washer", 0.200)];
        const PER_PERSON_L_DAY: f32 = 85.0; // drinking 5, kitchen 15, basin 5, shower 35, toilet 14, laundry 11
        const HOUSEHOLD: [&str; 6] = ["home_water_use", "kitchen_sink", "bath_sink", "shower", "toilet", "washer"];
        let l_day = |def: &MachineDef, u: crate::utilities::Utility, dir: crate::utilities::PortDir| -> f32 {
            def.derive_ports().iter().filter(|p| p.utility == u && p.dir == dir).map(|p| p.flow_lpm).sum::<f32>() * 1440.0
        };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        for (file, people) in [("home.ron", 3usize), ("home_solo.ron", 1usize)] {
            let home = MachineHome::load(&root.join(file)).unwrap_or_else(|| panic!("{file} parses"));
            let all = home.all_instances();
            let placed = |id: &str| all.iter().filter(|i| i.machine == id).count() as f32;
            for (id, share) in HOT_SHARE {
                let def = &home.catalog[id];
                let (hot, cold) = (l_day(def, crate::utilities::Utility::HotWater, crate::utilities::PortDir::In), l_day(def, crate::utilities::Utility::Water, crate::utilities::PortDir::In));
                assert!(hot > 0.0 && cold > 0.0, "{file}: {id} takes a hot and a cold feed ({hot} and {cold} L a day)");
                assert!((hot / (hot + cold) - share).abs() < 0.005, "{file}: {id} draws {:.1}% of its water hot, the measured share is {}%", hot / (hot + cold) * 100.0, share * 100.0);
            }
            let fixtures_hot: f32 = HOT_SHARE.iter().map(|(id, _)| placed(id) * l_day(&home.catalog[*id], crate::utilities::Utility::HotWater, crate::utilities::PortDir::In)).sum();
            let heater = &home.catalog["water_heater"];
            let (made, taken) = (l_day(heater, crate::utilities::Utility::HotWater, crate::utilities::PortDir::Out), l_day(heater, crate::utilities::Utility::Water, crate::utilities::PortDir::In));
            assert!((placed("water_heater") * made - fixtures_hot).abs() < 0.2, "{file}: the heater makes {made:.1} L a day hot, the fixtures draw {fixtures_hot:.1}");
            assert!((made - taken).abs() < 0.01, "{file}: the heater takes {taken:.1} L cold for {made:.1} hot");
            let household: f32 = HOUSEHOLD
                .iter()
                .map(|id| placed(id) * (l_day(&home.catalog[*id], crate::utilities::Utility::Water, crate::utilities::PortDir::In) + l_day(&home.catalog[*id], crate::utilities::Utility::HotWater, crate::utilities::PortDir::In)))
                .sum();
            println!("{file}: household {household:.1} L a day, {fixtures_hot:.1} of it hot ({:.0}%)", fixtures_hot / household * 100.0);
            assert!((household - PER_PERSON_L_DAY * people as f32).abs() < 0.5, "{file}: the household draws {household:.1} L a day, {people} x {PER_PERSON_L_DAY}");
            let hose = home.connections.iter().any(|c| c.kind == "hot_water" && all.iter().any(|i| i.id == c.from && i.machine == "water_heater") && all.iter().any(|i| i.id == c.to && i.machine == "washer"));
            assert!(hose, "{file}: a hot hose runs from the water heater to the washer");
        }
    }

    /// v0.664: a battery bank is STORAGE, not demand -- its inferred bidirectional bus terminal
    /// (a cable rating) must not inflate the power meter's kWh/day demand. Pre-fix, each shipped
    /// bank added max_discharge_w x 24 h (48 kWh/day of phantom demand per bank).
    #[test]
    fn utility_meters_do_not_count_batteries_as_demand() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 4000.0, max_charge_w: 2000.0, max_discharge_w: 2000.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("p1", "panel"), inst("b1", "batt"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let meters = home.utility_meters(4.5, MeterBasis::default());
        let power = meters.iter().find(|m| m.utility == "power").expect("a power meter exists");
        // Demand is ONLY the 100 W consumer (2.4 kWh/day) -- not 2.4 + the battery's 48.
        assert!((power.demand - 2.4).abs() < 1e-3, "battery must not count as demand: {}", power.demand);
        assert!((power.self_sufficiency - 1.0).abs() < 1e-6, "the home stays self-sufficient");
    }

    /// v0.664, homestead-solo-design.md gap #5: the grow-light power meter. No grow lights -> no
    /// report; a light inside the free solar headroom is GREEN; past the headroom is AMBER (the
    /// home eats battery reserves daily); enough lights that the draw ALONE exceeds the whole
    /// home's generation is RED. Exact thresholds asserted: a 1000 W panel at 4.5 sun-hours makes
    /// 4.5 kWh/day; the 100 W base load uses 2.4 (24 h) -> 2.1 kWh/day free headroom; each 100 W
    /// grow light draws 100 x 6 h = 0.6 kWh/day (the timer's hours).
    #[test]
    fn grow_light_meter_green_amber_red_thresholds() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        catalog.insert("grow_light".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 5, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("p1", "panel"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        // Zero grow lights -> no report (the meter row only appears once one is placed).
        assert!(home.grow_light_report(4.5, MeterBasis::default()).is_none(), "no lights -> no report");

        // (CHANGED 2026-09-26: a light runs the timer's 6 h a day, not 14, so
        // each is 0.6 kWh; the thresholds are the same, the light counts are
        // higher.) 1 light: 0.6 kWh/day <= 2.1 headroom -> GREEN.
        home.instances.push(inst("gl1", "grow_light"));
        let r = home.grow_light_report(4.5, MeterBasis::default()).expect("one light -> a report");
        assert_eq!(r.count, 1);
        assert!((r.watts - 100.0).abs() < 1e-3, "watts {}", r.watts);
        assert!((r.draw_kwh_day - 0.6).abs() < 1e-3, "draw {}", r.draw_kwh_day);
        assert!((r.headroom_kwh_day - 2.1).abs() < 1e-3, "headroom {}", r.headroom_kwh_day);
        assert!((r.generation_kwh_day - 4.5).abs() < 1e-3, "gen {}", r.generation_kwh_day);
        assert_eq!(r.verdict, GrowLightVerdict::WithinHeadroom);
        assert!(r.summary.contains("inside"), "{}", r.summary);

        // 4 lights: 2.4 > 2.1 headroom but <= 4.5 generated -> AMBER (daily reserve deficit).
        for id in ["gl2", "gl3", "gl4"] {
            home.instances.push(inst(id, "grow_light"));
        }
        let r = home.grow_light_report(4.5, MeterBasis::default()).expect("a report");
        assert_eq!(r.count, 4);
        assert_eq!(r.verdict, GrowLightVerdict::EatingReserves);
        assert!(r.summary.contains("battery reserves"), "{}", r.summary);

        // 8 lights: 4.8 kWh/day > the whole home's 4.5 generated -> RED.
        for id in ["gl5", "gl6", "gl7", "gl8"] {
            home.instances.push(inst(id, "grow_light"));
        }
        let r = home.grow_light_report(4.5, MeterBasis::default()).expect("a report");
        assert_eq!(r.count, 8);
        assert!((r.draw_kwh_day - 4.8).abs() < 1e-3, "draw {}", r.draw_kwh_day);
        assert_eq!(r.verdict, GrowLightVerdict::ExceedsGeneration);
        assert!(r.summary.contains("more than the whole home generates"), "{}", r.summary);
    }

    /// v0.664: the shipped catalogs must offer a PLACEABLE grow light (a Consumer in the palette),
    /// or the grow-light meter is unreachable in-app. Also pins the seed designs to ship with ZERO
    /// grow lights placed -- the reference homes grow under the sun; the meter is the player's
    /// discovery when they add one.
    #[test]
    fn shipped_catalogs_offer_a_placeable_grow_light() {
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join("machines")
                .join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            let def = home.catalog.get("grow_light").unwrap_or_else(|| panic!("{file} catalogs a grow_light"));
            assert!(
                matches!(def.power, Some(MachinePower::Consumer { watts, .. }) if watts > 0.0),
                "{file}: grow_light is a real electrical consumer"
            );
            assert!(
                home.grow_light_report(4.5, MeterBasis::default()).is_none(),
                "{file}: the seed design places no grow lights (sun-lit by design)"
            );
        }
    }

    /// Gardening depth, rung 2 (2026-09-26): the grow light in both shipped
    /// catalogs is the light FarmingSystem reads, so it must carry
    /// `lights_crops` (without it, placing one would draw 100 W and light
    /// nothing, the gap this closes), and nothing else may: a pump or a
    /// heater flagged by mistake would light the whole indoor garden. Grow
    /// lights are named the way `grow_light_report` finds them.
    #[test]
    fn shipped_grow_lights_light_crops_and_nothing_else_does() {
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join("machines")
                .join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (id, def) in &home.catalog {
                let is_grow_light = id == "grow_light" || id.starts_with("grow_light_");
                assert_eq!(
                    def.lights_crops, is_grow_light,
                    "{file}: `{id}` lights_crops should be {is_grow_light}"
                );
                if def.lights_crops {
                    assert!(
                        matches!(def.power, Some(MachinePower::Consumer { .. })),
                        "{file}: `{id}` lights crops, so it needs a Consumer power role to be switched on"
                    );
                }
            }
        }
    }

    /// Greenhouse humidity (2026-09-26): the exhaust fan in both shipped
    /// catalogs is the machine FarmingSystem reads as a fan, with its cited
    /// airflow (1604 CFM, 2,725 m3/h) and a 250 W Consumer power role (its
    /// controller scales that draw), and nothing else ventilates. The
    /// 3-person home places one in its greenhouse, which needs it
    /// (data/garden/humidity.ron); the one-person home, which does not,
    /// places none. Seen red by dropping the fan instance from home.ron.
    #[test]
    fn shipped_exhaust_fan_ventilates_and_only_the_family_home_places_one() {
        for (file, placed) in [("home.ron", 1usize), ("home_solo.ron", 0)] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (id, def) in &home.catalog {
                // The humidity fan, and since 2026-09-27 the two CO2 fans.
                let is_fan = id == "exhaust_fan" || id == "tent_co2_fan" || id == "room_co2_fan";
                assert_eq!(def.ventilation_m3_h > 0.0, is_fan, "{file}: `{id}` ventilates only if it is a fan");
                assert_eq!(def.co2_setpoint_ppm > 0.0, id.ends_with("co2_fan"), "{file}: `{id}` is switched on CO2 only if it is a CO2 fan");
                if id == "exhaust_fan" {
                    assert!((def.ventilation_m3_h - 2725.0).abs() < 1.0, "{file}: 1604 CFM");
                    assert!(
                        matches!(def.power, Some(MachinePower::Consumer { watts, .. }) if (watts - 250.0).abs() < 1e-3),
                        "{file}: a 250 W Consumer, so it can be switched on and shed"
                    );
                }
            }
            let fans: Vec<MachineInstance> =
                home.all_instances().into_iter().filter(|i| i.machine == "exhaust_fan").collect();
            assert_eq!(fans.len(), placed, "{file}: fans placed");
            assert!(fans.iter().all(|f| f.room == "room-greenhouse"), "{file}: in the greenhouse");
        }
    }

    /// The mushroom CO2 fans (2026-09-27): both shipped catalogs carry the tent
    /// fan (AC Infinity CLOUDLINE S4: 226 CFM = 384 m3/h, a 28 W Consumer,
    /// switched at 900 ppm) and the room fan (CLOUDLINE S6: 425 CFM = 722.1
    /// m3/h, 70 W, at 600 ppm); every mushroom rack has a tent fan at its spot,
    /// high enough to stand inside its tent, and each home's mushroom room one
    /// room fan, all cabled to a battery. Seen red by deleting home.ron's
    /// `mushfan` array (the six home racks then had none).
    #[test]
    fn every_shipped_mushroom_rack_has_a_wired_co2_fan() {
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (id, m3_h, w, set) in [("tent_co2_fan", 384.0, 28.0, 900.0), ("room_co2_fan", 722.1, 70.0, 600.0)] {
                let def = &home.catalog[id];
                assert!((def.ventilation_m3_h - m3_h).abs() < 0.1 && (def.co2_setpoint_ppm - set).abs() < 1e-3, "{file}: {id}");
                assert!(
                    matches!(def.power, Some(MachinePower::Consumer { watts, .. }) if (watts - w).abs() < 1e-3),
                    "{file}: {id} a {w} W Consumer, so it can be switched on and shed"
                );
                // 226 and 425 CFM at 1.69901 m3/h a CFM.
                assert!((def.ventilation_m3_h - (if w < 50.0 { 226.0 } else { 425.0 }) * 1.69901).abs() < 0.1, "{file}: {id} CFM");
            }
            let all = home.all_instances();
            let wired = |id: &str| home.connections.iter().any(|c| c.kind == "power" && c.to == id && c.from.starts_with("battery_"));
            for r in all.iter().filter(|i| i.machine == "mushroom_rack") {
                let fan = all
                    .iter()
                    .find(|f| f.machine == "tent_co2_fan" && f.offset.0 == r.offset.0 && f.offset.2 == r.offset.2)
                    .unwrap_or_else(|| panic!("{file}: no CO2 fan in rack {}'s tent", r.id));
                // Inside the tent: the medium's 1.9 m tall enclosure stands on the rack's floor.
                assert!(fan.offset.1 > r.offset.1 && fan.offset.1 < r.offset.1 + 1.9, "{file}: {} inside the tent", fan.id);
                assert!(wired(&fan.id), "{file}: {} cabled to a battery", fan.id);
            }
            let room_fans: Vec<_> = all.iter().filter(|i| i.machine == "room_co2_fan").collect();
            assert_eq!(room_fans.len(), 1, "{file}: one room fan");
            assert!(room_fans[0].room == "room-mushroom" && wired(&room_fans[0].id), "{file}: in the mushroom room, cabled");
        }
    }

    /// The mushroom racks' humidifiers (2026-09-26): both shipped catalogs
    /// carry two humidifiers and nothing else humidifies, each with its cited
    /// output and a Consumer power role (the controller scales that draw):
    /// the room unit (AC Infinity CLOUDFORGE T7, 1300 ml/h, 100 W), which
    /// neither home places now its racks fruit in tents, and the tent unit
    /// (CLOUDFORGE T3, 240 ml/h, 24 W), one placed on every mushroom rack
    /// (at the rack's spot, inside its tent) and cabled to a battery. Seen
    /// red by turning home.ron's `mushhum` array into another machine (the
    /// six home racks then had none).
    #[test]
    fn every_shipped_mushroom_rack_has_a_wired_tent_humidifier() {
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (id, def) in &home.catalog {
                let humidifier = id == "humidifier" || id == "tent_humidifier";
                assert_eq!(def.humidifies_l_h > 0.0, humidifier, "{file}: `{id}` humidifies only if it is a humidifier");
            }
            for (id, l_h, w) in [("humidifier", 1.3, 100.0), ("tent_humidifier", 0.24, 24.0)] {
                let def = &home.catalog[id];
                assert!((def.humidifies_l_h - l_h).abs() < 1e-6, "{file}: {id} {l_h} L/h");
                assert!(
                    matches!(def.power, Some(MachinePower::Consumer { watts, .. }) if (watts - w).abs() < 1e-3),
                    "{file}: {id} a {w} W Consumer, so it can be switched on and shed"
                );
            }
            let all = home.all_instances();
            assert!(!all.iter().any(|i| i.machine == "humidifier"), "{file}: no whole-room humidifier placed");
            let racks: Vec<&MachineInstance> = all.iter().filter(|i| i.machine == "mushroom_rack").collect();
            assert!(!racks.is_empty(), "{file}: mushroom racks");
            for r in racks {
                let hum = all
                    .iter()
                    .find(|h| h.machine == "tent_humidifier" && h.offset == r.offset)
                    .unwrap_or_else(|| panic!("{file}: no tent humidifier on rack {}", r.id));
                assert!(
                    home.connections.iter().any(|c| c.kind == "power" && c.to == hum.id && c.from.starts_with("battery_")),
                    "{file}: {} cabled to a battery",
                    hum.id
                );
            }
        }
    }

    /// Ship life support (2026-09-26): both shipped catalogs carry the air
    /// handler (Carrier 42CT size 14: 1,842 m3/h, a 325 W Consumer) and the CO2
    /// scrubber (the ISS CDRA: 4.74 kg a day, an 860 W Consumer shed last), and
    /// nothing else dehumidifies or scrubs. Each home places air handlers in its
    /// greenhouse and in its own air, and every one is cabled to a battery and
    /// piped onto the cistern's water island, so its condensate has a way back to
    /// the tanks. Seen red by deleting home.ron's line from air_handler_h1 to the
    /// cistern (it then stood on an island of its own).
    #[test]
    fn every_shipped_air_handler_is_cabled_and_piped_back_to_the_cistern() {
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (id, def) in &home.catalog {
                assert_eq!(def.dehumidifies_m3_h > 0.0, id == "air_handler", "{file}: `{id}` dehumidifies only if it is the air handler");
                assert_eq!(def.scrubs_co2_kg_day > 0.0, id == "air_recycler", "{file}: `{id}` scrubs only if it is the CO2 scrubber");
            }
            let ah = &home.catalog["air_handler"];
            assert!((ah.dehumidifies_m3_h - 1842.0).abs() < 1.0, "{file}: 1084 CFM");
            assert!(matches!(ah.power, Some(MachinePower::Consumer { watts, .. }) if (watts - 325.0).abs() < 1e-3), "{file}: 325 W");
            let sc = &home.catalog["air_recycler"];
            assert!((sc.scrubs_co2_kg_day - 4.74).abs() < 1e-6, "{file}: 4.74 kg a day");
            assert!(matches!(sc.power, Some(MachinePower::Consumer { watts, priority: 1, .. }) if (watts - 860.0).abs() < 1e-3), "{file}: 860 W, shed last");
            let all = home.all_instances();
            let handlers: Vec<&MachineInstance> = all.iter().filter(|i| i.machine == "air_handler").collect();
            assert!(handlers.iter().any(|h| h.room == "room-greenhouse"), "{file}: one in the greenhouse");
            assert!(handlers.iter().any(|h| h.room != "room-greenhouse"), "{file}: one in the home's own air");
            let islands = home.water_islands(&all);
            let cistern = islands.get("cistern_1").copied().unwrap_or_else(|| panic!("{file}: the cistern is on an island"));
            for h in handlers {
                assert!(
                    home.connections.iter().any(|c| c.kind == "power" && c.to == h.id && c.from.starts_with("battery_")),
                    "{file}: {} cabled to a battery",
                    h.id
                );
                assert_eq!(islands.get(&h.id), Some(&cistern), "{file}: {} piped onto the cistern's island", h.id);
            }
        }
    }

    /// v0.524 Stage 3: panel + battery sized for the night + a modest load passes every check.
    #[test]
    fn buildability_passes_a_balanced_home() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 2000.0, max_charge_w: 500.0, max_discharge_w: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "garage".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
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
            grown: Default::default(),
};
        // 1000W * 4.5h = 4500 Wh/day made vs 100W * 24h = 2400 used; night need = 100W * 19.5h =
        // 1950 Wh <= 2000 Wh battery, so every check passes.
        let report = home.buildability_report(4.5, MeterBasis::default());
        assert_eq!(report.worst(), CheckStatus::Pass, "balanced home passes: {:?}", report.checks);
    }

    /// v0.524 Stage 3: an under-sized battery warns (covers the day, not the night).
    #[test]
    fn buildability_warns_on_undersized_battery() {
        let mut catalog = BTreeMap::new();
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })));
        catalog.insert("batt".to_string(), def_with_power(Some(MachinePower::Battery { capacity_wh: 200.0, max_charge_w: 500.0, max_discharge_w: 500.0 })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "garage".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
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
            grown: Default::default(),
};
        let report = home.buildability_report(4.5, MeterBasis::default());
        assert_eq!(report.worst(), CheckStatus::Warn, "tiny battery warns: {:?}", report.checks);
    }

    /// v0.524 Stage 3: a connection to a missing machine fails the Wiring check.
    #[test]
    fn buildability_flags_a_dangling_connection() {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), test_def("box"));
        let home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![MachineInstance { id: "a".into(), machine: "box".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None }],
            arrays: Vec::new(),
            connections: vec![MachineConnection { from: "a".into(), to: "ghost".into(), kind: "power".into(), spec: None }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let report = home.buildability_report(4.5, MeterBasis::default());
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
        let report = home.buildability_report(4.5, MeterBasis::default());
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
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![
                MachineInstance { id: "b1".into(), machine: "box".into(), room: "garage".into(), offset: (1.0, 0.0, 2.0), rotation: 0.0, zone: "home".into(), screen_source: None },
                MachineInstance { id: "s1".into(), machine: "ball".into(), room: "garage".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None },
                MachineInstance { id: "ghost".into(), machine: "box".into(), room: "nowhere".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None },
            ],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
}
    }

    /// A single "home" zone at the world origin -- the pre-v0.754 single-box world, expressed in
    /// the new per-zone form, so the legacy box-mode tests keep asserting the same behavior.
    fn one_zone(w: f32, d: f32, h: f32) -> Vec<ZoneRect> {
        vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (w, d, h) }]
    }

    /// v0.754 (ship-superstructure increment A): a machine whose `zone` names a second zone stays
    /// inside THAT zone's footprint at that zone's origin -- not the home's -- and its y sits on
    /// that zone's deck. A stale zone id falls back to "home" deterministically. Since increment
    /// 1a the offset is ZONE-LOCAL: (5, 0, 5) in a zone at (70, 2, 5) stands at (75, 2, 10).
    #[test]
    fn machine_clamps_into_its_zones_footprint_at_that_zones_origin() {
        let mut home = pos_test_home();
        home.instances = vec![
            // At the commons zone's very corner: kept 0.3 m inside, at 70.3 x, 5.3 z.
            MachineInstance { id: "shop".into(), machine: "box".into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "commons".into(), screen_source: None },
            // Inside the commons footprint: zone origin + offset, y on the commons deck (y=2).
            MachineInstance { id: "stall".into(), machine: "box".into(), room: "g".into(), offset: (5.0, 0.0, 5.0), rotation: 0.0, zone: "commons".into(), screen_source: None },
            // A stale zone id: falls back to the "home" zone's footprint.
            MachineInstance { id: "lost".into(), machine: "box".into(), room: "g".into(), offset: (75.0, 0.0, 10.0), rotation: 0.0, zone: "deleted_zone".into(), screen_source: None },
        ];
        let zones = vec![
            ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (55.0, 89.0, 3.0) },
            ZoneRect { id: "commons".into(), origin: (70.0, 2.0, 5.0), size: (20.0, 30.0, 6.0) },
        ];
        let placed = home.placements(&std::collections::HashMap::new(), Some(&zones));
        let shop = placed.iter().find(|p| p.id == "shop").unwrap();
        assert!((shop.pos.0 - 70.3).abs() < 1e-5, "clamped to the commons' near x edge, got {}", shop.pos.0);
        assert!((shop.pos.2 - 5.3).abs() < 1e-5, "clamped to the commons' near z edge, got {}", shop.pos.2);
        assert_eq!(shop.floor_y, 2.0, "floor is the commons deck height");
        assert_eq!(shop.ceiling_y, 8.0, "ceiling is deck + zone height");
        let stall = placed.iter().find(|p| p.id == "stall").unwrap();
        assert_eq!(stall.pos, (75.0, 2.0, 10.0), "an in-footprint machine is unmoved, y on its deck");
        let lost = placed.iter().find(|p| p.id == "lost").unwrap();
        assert!(lost.pos.0 <= 55.0 - 0.3 + 1e-5 && lost.pos.2 <= 89.0 - 0.3 + 1e-5, "a stale zone id falls back to the home footprint");
        assert_eq!(lost.floor_y, 0.0);
    }

    /// Increment 1a (docs/design/ship-homes-and-logistics.md): the Commons' 11 rows live in
    /// data/machines/ship.ron, Commons-local. home.ron links it, so loading home.ron gives the
    /// same machines as before the split, and on the ship assembled at p1 every Commons machine
    /// stands exactly where it stood (its old absolute position). home_solo.ron does not link it.
    /// Saving writes the household's part to the home file and the ship's part only through
    /// `save_ship_part`. Red check, run: dropping `merge_ship_machines` from `load` fails the
    /// first assertion (the Commons machines are missing).
    #[test]
    fn the_commons_machines_live_in_the_ship_file_and_merge_back_in() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let machines = data.join("machines");
        let commons_ids = ["mushhum_c1", "mushfan_c1", "aqua_c1", "composter_c1", "mush_c1", "apoth_c1", "apoth_c2", "market_c1", "market_c2", "market_c3"];
        let merged = MachineHome::load(&machines.join("home.ron")).expect("home.ron loads");
        let ids: Vec<String> = merged.all_instances().into_iter().map(|i| i.id).collect();
        for id in commons_ids.iter().chain(["ctower_0", "ctower_8"].iter()) {
            assert!(ids.iter().any(|i| i == id), "{id} is merged in from ship.ron");
        }
        let home_only = MachineHome::load_file(&machines.join("home.ron")).expect("parses");
        assert_eq!(home_only.ship_machines.as_deref(), Some("ship.ron"));
        assert!(home_only.all_instances().iter().all(|i| i.zone == "home"), "home.ron holds only home rows");
        let ship_only = MachineHome::load_file(&machines.join("ship.ron")).expect("parses");
        assert_eq!(ship_only.instances.len() + ship_only.arrays.len(), 11, "the 11 Commons rows");
        assert_eq!(ship_only.connections.len(), 5, "the 5 connections that touch them");
        assert!(ship_only.all_instances().iter().all(|i| i.zone == "commons"));
        let solo = MachineHome::load(&machines.join("home_solo.ron")).expect("home_solo.ron loads");
        assert!(solo.ship_machines.is_none() && solo.all_instances().iter().all(|i| i.zone == "home"), "the solo home loads without them, as before");

        // Every Commons machine stands where it stood before the split.
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, None).expect("assembles");
        let placed = merged.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()));
        for (id, was) in [("mush_c1", (76.5, 0.0, 49.0)), ("aqua_c1", (76.5, 0.0, 44.0)), ("market_c1", (97.0, 0.0, 32.0)), ("market_c3", (97.0, 0.0, 60.0)), ("ctower_0", (79.0, 0.0, 44.0)), ("apoth_c2", (86.5, 0.0, 52.0))] {
            let p = placed.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id} is placed"));
            assert!(
                (p.pos.0 - was.0).abs() < 1e-4 && (p.pos.1 - was.1).abs() < 1e-4 && (p.pos.2 - was.2).abs() < 1e-4,
                "{id} stands at {:?}, it stood at {was:?}",
                p.pos
            );
        }

        // Saving splits: the household part to home.ron, the ship part only via save_ship_part.
        let dir = std::env::temp_dir().join(format!(
            "hos_ship_machines_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let mdir = dir.join("machines");
        std::fs::create_dir_all(&mdir).unwrap();
        for f in ["home.ron", "ship.ron"] {
            std::fs::copy(machines.join(f), mdir.join(f)).unwrap();
        }
        let ship_bytes = std::fs::read(mdir.join("ship.ron")).unwrap();
        let mut layout = MachineHome::load(&mdir.join("home.ron")).expect("loads");
        layout.instances.iter_mut().find(|i| i.id == "market_c2").unwrap().offset.0 = 20.0;
        layout.save(&mdir.join("home.ron")).expect("saves");
        let written = MachineHome::load_file(&mdir.join("home.ron")).expect("parses");
        assert!(written.all_instances().iter().all(|i| i.zone == "home"), "a save never copies the Commons into the home file");
        assert_eq!(written.connections.len(), home_only.connections.len(), "only the household's connections");
        assert_eq!(std::fs::read(mdir.join("ship.ron")).unwrap(), ship_bytes, "save() leaves the ship file alone");
        layout.save_ship_part(&mdir.join("home.ron")).expect("writes the ship part");
        let back = MachineHome::load(&mdir.join("home.ron")).expect("loads");
        assert_eq!(back.instances.iter().find(|i| i.id == "market_c2").unwrap().offset.0, 20.0, "the ship part was written");
        assert_eq!(back.all_instances().len(), merged.all_instances().len(), "nothing lost or doubled");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Outside the Dev mode the ship's machines are read-only (`ShipPart::locked`), because
    /// only a Dev save writes data/machines/ship.ron: an edit to a Commons stall in Normal mode
    /// would be dropped on the next load without a word (the critic's review of increment 1a).
    /// Locked, the ship's rows (an array cell too) cannot be removed, wired, unwired or given a
    /// conduit edge, while the household's own machines edit as before. Unlocked (the Dev mode)
    /// they edit freely. A layout that links no ship file has nothing to lock.
    /// Red check, run: deleting the `is_locked` guard in `remove_instance` fails the first
    /// "still there" assertion.
    #[test]
    fn the_ships_machines_are_read_only_outside_the_dev_mode() {
        let machines = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        let mut layout = MachineHome::load(&machines.join("home.ron")).expect("home.ron loads");
        layout.ship_part.locked = true;
        let has = |h: &MachineHome, id: &str| h.all_instances().iter().any(|i| i.id == id);
        assert!(layout.is_locked("market_c2") && layout.is_locked("ctower_0"), "a Commons row and an array cell are the ship's");
        assert!(!layout.is_locked("battery_7"), "the household's battery is not");
        assert!(layout.locked_ids().contains("ctower_8") && !layout.locked_ids().contains("battery_7"));

        layout.remove_instance("market_c2");
        assert!(has(&layout, "market_c2"), "a locked ship row is still there after a remove");
        assert!(!layout.add_connection("battery_7", "market_c1", "power"), "no new wire to a ship row");
        let to_ship = layout.connections.iter().position(|c| c.from == "battery_7" && c.to == "mushhum_c1").expect("the battery feeds the Commons humidifier");
        assert!(!layout.remove_connection(to_ship), "that wire is the ship's too");
        assert!(!layout.remove_connection_between("mushhum_c1", "battery_7"), "in either direction");
        assert!(!layout.add_conduit_edge(ConduitEnd::Machine("aqua_c1".into()), ConduitEnd::Machine("battery_7".into()), "power"));

        // The household's own machines still edit.
        let home_wire = layout.connections.iter().position(|c| !layout.connection_locked(&c.from, &c.to)).expect("a household wire");
        assert!(layout.remove_connection(home_wire), "a household wire comes out");
        assert!(layout.add_connection("battery_7", "battery_8", "power") || layout.connections.iter().any(|c| c.from == "battery_7" && c.to == "battery_8"));
        layout.remove_instance("battery_8");
        assert!(!has(&layout, "battery_8"), "a household machine is removed");

        // Dev: unlocked, the ship's rows edit.
        layout.ship_part.locked = false;
        assert!(!layout.is_locked("market_c2") && layout.locked_ids().is_empty());
        layout.remove_instance("market_c2");
        assert!(!has(&layout, "market_c2"), "in the Dev mode a ship row can be removed");

        // A self-contained file has no ship part to lock.
        let mut solo = MachineHome::load(&machines.join("home_solo.ron")).expect("home_solo.ron loads");
        solo.ship_part.locked = true;
        let first = solo.instances[0].id.clone();
        assert!(!solo.is_locked(&first) && solo.locked_ids().is_empty());
    }

    /// A run between two zones measures in SHIP metres. Machine offsets are zone-local since
    /// increment 1a, so the home's battery (home-local 3, 34.2) and the Commons humidifier it
    /// feeds (Commons-local 11.5, 29) are 75 m apart on the ship, not the 10 m their offsets
    /// differ by. Then the report: a 1 kW load on pinned 12 AWG passes 2 m away in the same zone
    /// and fails once the load's zone is 500 m off, which the report only sees when it is given
    /// the zones. Red check, run: measuring `from.offset` to `to.offset` in the Conduits check
    /// again (the 1a code) fails the "fails 500 m away" assertion.
    #[test]
    fn a_run_between_zones_measures_in_ship_metres() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let layout = MachineHome::load(&data.join("machines").join("home.ron")).expect("home.ron loads");
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("assembles");
        let zones = ship.zone_rects();
        let real = layout.run_length("battery_7", "mushhum_c1", Some(&zones)).expect("both are placed");
        let expect = (73.5f32 * 73.5 + 14.8 * 14.8).sqrt(); // (3, 34.2) to (65 + 11.5, 20 + 29)
        assert!((real - expect).abs() < 0.01, "the run is {real} m, it is {expect} m on the ship");
        let frames_mixed = layout.run_length("battery_7", "mushhum_c1", None).unwrap();
        assert!(frames_mixed < 10.0, "without the zones the offsets alone say {frames_mixed} m");

        let mut home = wired_pair(
            def_with_power(Some(MachinePower::Generator { watts: 5000.0, fuel_lph: 0.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 1000.0, priority: 1, idle_watts: None, average_watts: None })),
            2.0,
            Some("cu_awg12"),
        );
        let conduits = |r: BuildabilityReport| r.checks.into_iter().find(|c| c.name == "Conduits").expect("a Conduits check");
        let near = vec![
            ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (10.0, 10.0, 3.0) },
            ZoneRect { id: "far".into(), origin: (500.0, 0.0, 0.0), size: (10.0, 10.0, 3.0) },
        ];
        assert_eq!(conduits(home.buildability_report_in(4.5, MeterBasis::default(), Some(&near))).status, CheckStatus::Pass, "2 m apart in one zone");
        home.instances[1].zone = "far".into();
        let far = conduits(home.buildability_report_in(4.5, MeterBasis::default(), Some(&near)));
        assert_eq!(far.status, CheckStatus::Fail, "500 m away a 12 AWG run drops too much: {}", far.detail);
    }

    /// Merge and split mirror each other for EVERY part of the ship's machine file: a loop, a
    /// conduit node and a conduit edge authored in the ship file go back there on a save and stay
    /// out of the home file, and the household's own node stays home. Before the fix the merge
    /// brought the ship's nodes in and the split gave every node to the home file (and dropped the
    /// ship's loops). Red check, run: returning `Vec::new()` for the ship part's conduit nodes
    /// again fails "the ship's node went back to the ship file".
    #[test]
    fn the_ships_loops_and_conduit_graph_round_trip_to_the_ship_file() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        let dir = std::env::temp_dir().join(format!(
            "hos_ship_conduits_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(src.join("home.ron"), dir.join("home.ron")).unwrap();
        let mut ship = MachineHome::load_file(&src.join("ship.ron")).expect("ship.ron parses");
        ship.conduit_nodes.push(ConduitNode { id: "commons_main".into(), pos: (70.0, 0.0, 30.0), tier: 0, kind: "water".into(), grid_tie: true });
        ship.conduit_edges.push(ConduitEdge { from: ConduitEnd::Node("commons_main".into()), to: ConduitEnd::Machine("aqua_c1".into()), kind: "water".into() });
        ship.loops.push(HomeLoop { name: "Commons water".into(), demand: "d".into(), supply: "s".into(), closes: true, weakest: false, note: String::new(), food_demand_kcal: None });
        ship.write_ron(&dir.join("ship.ron")).unwrap();

        let mut layout = MachineHome::load(&dir.join("home.ron")).expect("loads merged");
        assert!(layout.conduit_nodes.iter().any(|n| n.id == "commons_main") && layout.loops.iter().any(|l| l.name == "Commons water"), "merged in");
        layout.conduit_nodes.push(ConduitNode { id: "home_main".into(), pos: (5.0, 0.0, 5.0), tier: 0, kind: "water".into(), grid_tie: false });
        layout.save(&dir.join("home.ron")).expect("saves the household part");
        layout.save_ship_part(&dir.join("home.ron")).expect("saves the ship part");

        let ship_back = MachineHome::load_file(&dir.join("ship.ron")).expect("parses");
        let home_back = MachineHome::load_file(&dir.join("home.ron")).expect("parses");
        assert!(ship_back.conduit_nodes.iter().any(|n| n.id == "commons_main"), "the ship's node went back to the ship file");
        assert!(!home_back.conduit_nodes.iter().any(|n| n.id == "commons_main"), "and not into the home file");
        assert_eq!(ship_back.conduit_edges.len(), 1, "the ship's edge went back to the ship file");
        assert!(ship_back.loops.iter().any(|l| l.name == "Commons water"), "the ship's loop went back to the ship file");
        assert!(!home_back.loops.iter().any(|l| l.name == "Commons water"), "and not into the home file");
        assert!(home_back.conduit_nodes.iter().any(|n| n.id == "home_main"), "the household's own node stays home");
        assert!(!ship_back.conduit_nodes.iter().any(|n| n.id == "home_main"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// v0.525/v0.538: in SHIP mode (zones=None) placements() resolves room center + offset,
    /// floor-relative y, lifts spheres, and SKIPS a machine whose room has no geometry.
    #[test]
    fn placements_ship_mode_is_room_relative_and_skips() {
        let home = pos_test_home();
        let mut rooms = std::collections::HashMap::new();
        rooms.insert("garage".to_string(), RoomGeom { center_x: 10.0, center_z: 20.0, floor_y: 5.0, ceiling_y: 8.0 });
        let placed = home.placements(&rooms, None);
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
        let placed = home.placements(&rooms, Some(&one_zone(55.0, 89.0, 3.0)));
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
        home.instances = vec![MachineInstance { id: "solar".into(), machine: "box".into(), room: "garage".into(), offset: (-7.0, 0.0, -22.0), rotation: 0.0, zone: "home".into(), screen_source: None }];
        let placed = home.placements(&std::collections::HashMap::new(), Some(&one_zone(55.0, 89.0, 3.0)));
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
        let placed = home.placements(&std::collections::HashMap::new(), Some(&one_zone(55.0, 89.0, 3.0)));
        assert_eq!(placed.len(), home.all_instances().len(), "box mode skips nothing -- the seed home renders fully");
    }

    /// v0.639: the shipped home.ron has EXACTLY ONE "drone_hangar" instance, and it resolves to a
    /// real placement in box mode -- the same lookup `lib.rs::hangar_placement` performs each frame
    /// to park the mining-drone visual on the pad. This is the renderer-free half of the dock/undock
    /// feature: it proves the hangar the drone docks at actually exists and resolves, independent of
    /// wgpu/the live ECS Drone state (which needs a real World + can't run in this pure data module).
    #[test]
    fn shipped_home_has_one_resolvable_drone_hangar() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let hangars: Vec<_> = home.all_instances().into_iter().filter(|i| i.machine == "drone_hangar").collect();
        assert_eq!(hangars.len(), 1, "v1 assumes exactly one drone hangar; update the visual's doc comment if this ever changes");
        let hangar_id = hangars[0].id.clone();
        // Box mode (the live HomeStructure home) never skips a machine, so this must resolve.
        let placed = home.placements(&std::collections::HashMap::new(), Some(&one_zone(55.0, 89.0, 3.0)));
        let hangar_placement = placed.iter().find(|p| p.id == hangar_id).expect("drone hangar resolves to a placement");
        assert_eq!(hangar_placement.shape, "box", "the hangar pad renders as its authored box shape");
    }

    #[test]
    fn drone_dock_visibility_tracks_drone_active_flag() {
        // Calls the REAL crate-level fn the render call site in src/lib.rs also calls (moved to
        // crate root and out of the native-only module 2026-07-01 specifically so this test can
        // reach it without pulling in rendering/ECS deps; a hand-copied duplicate here previously
        // could not have caught a regression in the real gate, this can).
        use crate::drone_dock_visible;
        // Docked: no drone out, in gameplay, machines not decluttered away.
        assert!(drone_dock_visible(false, false, false), "no drone in flight -> docked model shows");
        // Undocked: a drone launched (Outbound/Mining/Returning) -> gui_state.drone_active is true.
        assert!(!drone_dock_visible(false, false, true), "a drone is out -> the pad reads empty");
        // Never shown in the character showroom, regardless of drone state.
        assert!(!drone_dock_visible(true, false, false));
        // Never shown while the "Machine" type is decluttered away in the build editor.
        assert!(!drone_dock_visible(false, true, false));
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
        let a = std::env::temp_dir().join(format!("humanity_home_det_a_{}.ron", std::process::id()));
        let b = std::env::temp_dir().join(format!("humanity_home_det_b_{}.ron", std::process::id()));
        home.save(&a).expect("save a");
        home.save(&b).expect("save b");
        let ta = std::fs::read_to_string(&a).unwrap();
        let tb = std::fs::read_to_string(&b).unwrap();
        assert_eq!(ta, tb, "two saves of the same home must be byte-identical");
        // And a reload then re-save is identical too (round-trip stability).
        let reloaded = MachineHome::load(&a).expect("reload");
        let c = std::env::temp_dir().join(format!("humanity_home_det_c_{}.ron", std::process::id()));
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
        let tmp = std::env::temp_dir().join(format!("humanity_home_header_{}.ron", std::process::id()));
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
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![
                MachineInstance { id: "s1".into(), machine: "src".into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None },
                MachineInstance { id: "l1".into(), machine: "load".into(), room: "g".into(), offset: (gap, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None },
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
            grown: Default::default(),
}
    }

    /// derive_ports infers an electrical port from the `power` role, explicit ports override it, and
    /// the load helper sums IN + bidirectional electrical ports.
    #[test]
    fn derive_ports_infers_from_power_and_explicit_wins() {
        let consumer = def_with_power(Some(MachinePower::Consumer { watts: 200.0, priority: 1, idle_watts: None, average_watts: None }));
        assert_eq!(consumer.derive_ports().len(), 1, "a consumer infers one IN port");
        assert_eq!(consumer.electrical_load_watts(), 200.0);
        let panel = def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None }));
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
            def_with_power(Some(MachinePower::Solar { peak_watts: 1000.0, average_watts: None })),
            def_with_power(Some(MachinePower::Consumer { watts: 120.0, priority: 1, idle_watts: None, average_watts: None })),
            2.0,
            None,
        );
        let report = home.buildability_report(4.5, MeterBasis::default());
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Pass, "120 W over 2 m auto-sizes: {}", conduit.detail);
    }

    /// A pinned, undersized cable feeding a heavy load FAILS -- "not all cables can handle all power".
    #[test]
    fn buildability_conduits_undersized_pinned_cable_fails() {
        let home = wired_pair(
            def_with_power(Some(MachinePower::Generator { watts: 5000.0, fuel_lph: 0.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 3000.0, priority: 1, idle_watts: None, average_watts: None })),
            1.0,
            Some("cu_awg14"), // 15 A cable; 3000 W @ 120 V = 25 A -> over ampacity
        );
        let report = home.buildability_report(4.5, MeterBasis::default());
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Fail, "25 A on a 15 A cable fails: {}", conduit.detail);
    }

    /// A pinned cable id that isn't in the registry FAILS the check (an AI/hand edit can't reference a
    /// nonexistent cable and have it silently pass).
    #[test]
    fn buildability_conduits_unknown_cable_id_fails() {
        let home = wired_pair(
            def_with_power(Some(MachinePower::Generator { watts: 500.0, fuel_lph: 0.0 })),
            def_with_power(Some(MachinePower::Consumer { watts: 200.0, priority: 1, idle_watts: None, average_watts: None })),
            1.0,
            Some("unobtainium_42"),
        );
        let report = home.buildability_report(4.5, MeterBasis::default());
        let conduit = report.checks.iter().find(|c| c.name == "Conduits").expect("a Conduits check exists");
        assert_eq!(conduit.status, CheckStatus::Fail, "unknown cable id fails: {}", conduit.detail);
    }

    /// v0.621 telecom Stage 2: a DATA run sized to a wired medium PASSES, and so does the same run on
    /// WiFi, which carries the bandwidth over the range (2026-09-27: WiFi no longer raises an RF
    /// warning, removed with the Wi-Fi crop harm). A run beyond WiFi's range still fails. Builds the
    /// uplink -> server pair the Data-links check validates.
    #[test]
    fn buildability_data_links_wired_and_wifi_pass_on_bandwidth_and_range() {
        let mut uplink = test_def("box");
        uplink.ports = vec![crate::utilities::Port::data_out(1000.0)];
        let mut server = test_def("box");
        server.ports = vec![crate::utilities::Port::data_in(100.0)];
        let mut catalog = BTreeMap::new();
        catalog.insert("uplink".to_string(), uplink);
        catalog.insert("server".to_string(), server);
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("u", "uplink"), inst("s", "server")],
            arrays: Vec::new(),
            connections: vec![MachineConnection { from: "u".into(), to: "s".into(), kind: "data".into(), spec: Some("eth_cat6".into()) }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let wired = home.buildability_report(4.5, MeterBasis::default());
        let d = wired.checks.iter().find(|c| c.name == "Data links").expect("a Data links check");
        assert_eq!(d.status, CheckStatus::Pass, "wired Cat6 carries 100 Mbps: {}", d.detail);

        // Swap to WiFi: it carries the bandwidth over this short run, so it passes like the cable.
        home.connections[0].spec = Some("wifi_6".to_string());
        let wifi = home.buildability_report(4.5, MeterBasis::default());
        let d2 = wifi.checks.iter().find(|c| c.name == "Data links").unwrap();
        assert_eq!(d2.status, CheckStatus::Pass, "WiFi carries 100 Mbps over a short run: {}", d2.detail);
        assert!(!d2.detail.contains("RF"), "no RF warning on a WiFi link: {}", d2.detail);

        // The genuine range check stays: 60 m is twice WiFi's 30 m reach.
        home.instances[1].offset = (60.0, 0.0, 0.0);
        let far = home.buildability_report(4.5, MeterBasis::default());
        let d3 = far.checks.iter().find(|c| c.name == "Data links").unwrap();
        assert_eq!(d3.status, CheckStatus::Fail, "WiFi out of range fails: {}", d3.detail);
    }

    /// The shipped seed home's data link (uplink -> server over Cat6) sizes OK (no FAIL).
    #[test]
    fn buildability_seed_home_data_links_are_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        let home = MachineHome::load(&path).expect("home.ron parses");
        let report = home.buildability_report(4.5, MeterBasis::default());
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
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 100.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("b1", "batt"), inst("l1", "load")],
            arrays: Vec::new(),
            // load wired to a battery that itself reaches NO generator -> the load can't run.
            connections: vec![MachineConnection { from: "b1".into(), to: "l1".into(), kind: "power".into(), spec: None }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
};
        let circuit = home.power_circuit_check(&home.all_instances()).expect("electrical machines -> a circuit check");
        assert_eq!(circuit.status, CheckStatus::Fail, "battery-only load fails: {}", circuit.detail);
        // Now wire a panel onto the same bus -> the load traces to generation -> Pass.
        home.catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 800.0, average_watts: None })));
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
        catalog.insert("panel".to_string(), def_with_power(Some(MachinePower::Solar { peak_watts: 500.0, average_watts: None })));
        catalog.insert("load".to_string(), def_with_power(Some(MachinePower::Consumer { watts: 80.0, priority: 1, idle_watts: None, average_watts: None })));
        let inst = |id: &str, m: &str| MachineInstance { id: id.into(), machine: m.into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None };
        let mut home = MachineHome {
            ship_machines: None,
            ship_part: Default::default(),
            catalog,
            instances: vec![inst("p1", "panel"), inst("l1", "load")],
            arrays: Vec::new(),
            connections: Vec::new(),
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
            grown: Default::default(),
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
        // The household's taps are a NON-powered demand that exceeds rain -> drains the cistern when
        // the powered pump stops. This is what makes the consequence visible. Since 2026-09-27 the
        // household is carved into its fixtures (the drinking tap keeps 15 L a day), so the
        // unpowered demand is the drinking tap plus every fixture without a pump or an element.
        let household = home.catalog.get("home_water_use").expect("a household-water type");
        assert!(household.water_demand_lpm() > 0.0, "household draws water");
        assert!(!household.draws_power(), "household tap is NOT power-gated (taps flow in a blackout)");
        let tap: f32 = all
            .iter()
            .filter_map(|i| home.catalog.get(&i.machine))
            .filter(|d| !d.draws_power() && !d.irrigates && d.water_production_lpm() <= 0.0)
            .map(|d| d.water_demand_lpm())
            .sum();
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
        let report = home.buildability_report(4.5, MeterBasis::default());
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
        let report = home.buildability_report(4.5, MeterBasis::default());
        // If the seed has power runs at all, the Conduits check must not FAIL (Pass or Warn is fine).
        if let Some(conduit) = report.checks.iter().find(|c| c.name == "Conduits") {
            assert_ne!(conduit.status, CheckStatus::Fail, "seed conduits must be sizable: {}", conduit.detail);
        }
    }

    /// The one-person `home_solo.ron` variant (docs/design/homestead-solo-design.md) parses,
    /// and every instance's `machine` id resolves against its own catalog -- guards against a
    /// typo'd machine id silently rendering nothing.
    #[test]
    fn home_solo_variant_parses_and_all_instances_resolve() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home_solo.ron");
        let home = MachineHome::load(&path).expect("home_solo.ron parses");
        let all = home.all_instances();
        assert!(!all.is_empty(), "solo home has placed machines");
        for inst in &all {
            assert!(
                home.catalog.contains_key(&inst.machine),
                "instance '{}' references unknown machine id '{}'",
                inst.id,
                inst.machine
            );
        }
    }

    /// The solo home's power network is fully wired (every load traces to generation) -- the
    /// same "no magic transmission" guarantee the 3-person seed home holds.
    #[test]
    fn home_solo_variant_power_circuit_is_connected() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home_solo.ron");
        let home = MachineHome::load(&path).expect("home_solo.ron parses");
        let report = home.buildability_report(4.5, MeterBasis::default());
        let circuit = report.checks.iter().find(|c| c.name == "Power circuit").expect("the solo home has electrical machines");
        assert_ne!(circuit.status, CheckStatus::Fail, "solo home power must be fully wired: {}", circuit.detail);
    }
}

/// The camera render size (`MachineDef::camera_px`, 2026-09-18): its default,
/// its clamp, and that it survives a RON round trip both when written out
/// and when left out of the file.
#[cfg(test)]
mod camera_px_tests {
    use super::*;

    /// A def with a camera resolves the default 640 x 360 when the file says
    /// nothing, and a zero side is clamped to 1 rather than making a
    /// zero-sized texture.
    #[test]
    fn camera_px_defaults_to_640_by_360_and_clamps_zero() {
        let mut p = PlacedMachine {
            id: "camera_post_1".into(),
            room: "garden".into(),
            pos: (0.0, 0.0, 0.0),
            top_y: 1.7,
            floor_y: 0.0,
            ceiling_y: 3.0,
            shape: "box".into(),
            size: (0.14, 1.7, 0.14),
            color: (0.2, 0.2, 0.22),
            label: "Camera post".into(),
            stats: Vec::new(),
            rotation: 0.0,
            model: None,
            screen: None,
            camera: Some((0.0, -8.0, 70.0)),
            camera_px: CAMERA_PX_DEFAULT,
        };
        assert_eq!(CAMERA_PX_DEFAULT, (640, 360));
        assert_eq!(p.camera_pose().unwrap().px, (640, 360));
        p.camera_px = (0, 0);
        assert_eq!(p.camera_pose().unwrap().px, (1, 1), "a zero side clamps to 1");
        p.camera_px = (320, 180);
        assert_eq!(p.camera_pose().unwrap().px, (320, 180));
    }

    /// A machine def parsed from RON without `camera_px` gets the default;
    /// one that names it keeps it; and a def written back out and re-read
    /// carries the same value (the serde round trip).
    #[test]
    fn camera_px_round_trips_through_ron() {
        // The smallest def RON accepts (every other field is defaulted).
        let bare = r#"(shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.22), camera: Some((0.0, -8.0, 70.0)))"#;
        let def: MachineDef = ron::from_str(bare).expect("a bare camera def parses");
        assert_eq!(def.camera_px, CAMERA_PX_DEFAULT, "absent camera_px reads the default");

        let named = r#"(shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.22), camera: Some((0.0, -8.0, 70.0)), camera_px: (320, 180))"#;
        let def: MachineDef = ron::from_str(named).expect("a def naming camera_px parses");
        assert_eq!(def.camera_px, (320, 180));

        let out = ron::to_string(&def).expect("a def serialises");
        assert!(out.contains("camera_px"), "the field is written out: {out}");
        let back: MachineDef = ron::from_str(&out).expect("the written def parses again");
        assert_eq!(back.camera_px, (320, 180), "the value survives the round trip");
    }
}
