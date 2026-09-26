//! The Garden panel's value types: a growing crop for its card, the pests
//! and diseases in each grow area and the air it grows in, and the garden
//! settings and requests carried between the panel and the farming system
//! (pests, soil pH, pollination, humidity).
//!
//! Moved VERBATIM out of `gui/state_types.rs` (2026-09-26) when the garden
//! rungs pushed that file past its line budget in tests/file_size_ratchet.rs.
//! Re-exported by `state_types` (`pub use garden::*`), so every
//! `crate::gui::GuiCrop` spelling resolves exactly as before.

/// The pests in one grow area, for the Garden panel (2026-09-26,
/// systems::farming::pests): each pest with its level and its controls in
/// the IPM order (gentlest first), and the releases still working there.
#[derive(Debug, Clone, Default)]
pub struct GuiAreaPests {
    /// The grow area's id (a crop's `tower_id`; "" for hand-planted crops).
    pub area: String,
    /// (pest name, level 0..1, controls as (id, name, note)).
    pub pests: Vec<(String, f32, Vec<(String, String, String)>)>,
    /// (release name, garden days left).
    pub releases: Vec<(String, f32)>,
}

/// The Garden panel's pests and soil pH, and what the player just chose.
#[derive(Debug, Clone, Default)]
pub struct GardenPests {
    pub areas: Vec<GuiAreaPests>,
    /// (area, control id), carried to the farming system next frame.
    pub pending: Option<(String, String)>,
    /// Soil pH (farming::soil_ph): Settings "Soil pH: Off" (saved as
    /// AppConfig::soil_ph), the amendments as (id, label), and the one the
    /// player chose as (area, amendment id), carried next frame.
    pub soil_ph_off: bool,
    pub ph_amendments: Vec<(String, String)>,
    pub ph_pending: Option<(String, String)>,
    /// Pollination (2026-09-26, farming::pollination): Settings "Off" (the
    /// config's `pollination` inverted, so the derived default is On), the
    /// indoor areas whose flowers wait for a pollinator, and the area the
    /// player just chose to hand-pollinate.
    pub pollination_off: bool,
    pub pollinate_areas: Vec<String>,
    pub pollinate_pending: Option<String>,
    /// Picking over a season (2026-09-26, farming::picking): Settings
    /// "Realistic" (saved as AppConfig::picking_realistic; Forgiving is the
    /// default), and the crop the player chose to clear from its plot.
    pub picking_realistic: bool,
    pub clear_pending: Option<u64>,
    /// The air each grow area grows in (2026-09-26, farming::humidity): (area
    /// tag, its line for the panel, how humid: 0 fine, 1 above the fans'
    /// setpoint, 2 at the damp diseases' line). The line names the grow room
    /// and what its exhaust fans are doing.
    pub air: Vec<(String, String, u8)>,
}

/// A growing crop for GUI display (synced from the ECS each frame).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiCrop {
    /// hecs entity bits — used to target water/harvest commands.
    pub entity_bits: u64,
    pub name: String,
    pub stage: String,
    /// Growth progress 0..1 (by stage index).
    pub progress: f32,
    pub water: f32,
    pub health: f32,
    /// Average health over the growing season, 0..1 (2026-09-26): what sets
    /// the yield (farming::season_health). Current health recovers fast.
    pub season_health: f32,
    pub mature: bool,
    pub dead: bool,
    /// The tower this crop belongs to (its config id), if planted via a tower.
    pub tower_id: Option<String>,
    /// Which slot of the tower this crop occupies (0-based), for the slot view.
    pub tower_slot: Option<u32>,
    /// Plants in this crop's unit: 1 in a tower cup, as many as fit at the
    /// crop's spacing in a bed plot (farming::units, 2026-09-26).
    pub plants: u32,
    /// Plant-def reference data (from plants.csv) for the crop card: grams of
    /// N, P2O5 and K2O each kg of harvest carries out of the soil (the crop's
    /// cited removal columns, else its index read against the anchor;
    /// farming::soil::removal_per_kg), the UNIT's daily water need (L, per
    /// plant x plants), and the tolerated temperature window (Celsius). 0 when
    /// the species is unknown.
    pub n: f32,
    pub p: f32,
    pub k: f32,
    pub water_per_day: f32,
    pub temp_min: f32,
    pub temp_max: f32,
    /// What is lighting it now (farming::lighting::light_word).
    pub light: String,
    /// How much light it needs a day (farming::lighting::light_need_word).
    pub light_need: String,
    /// Grams of N, P2O5 and K2O the crop's unit holds, and what its season
    /// needs (2026-09-26, farming::soil).
    pub soil: [f32; 3],
    pub need: [f32; 3],
    /// The scarcest nutrient, when the crop is short of one.
    pub short_of: Option<String>,
    /// Its unit's pH (None: pH off or not modelled), its plants.csv window,
    /// the health cap pH sets, and whether a tower holds it (farming::soil_ph).
    pub ph: Option<f32>,
    pub ph_window: [f32; 2],
    pub ph_cap: f32,
    pub ph_held: bool,
    /// Its "Pollination" card row (farming::pollination::GuiView); "" = none.
    pub pollination: String,
    /// Its "Picking" card row (farming::picking::GuiView, 2026-09-26): how a
    /// crop picked over a season stands; "" for a crop harvested once.
    pub picking: String,
    /// Its "Humidity" card row: the air it grows in against its plants.csv
    /// window, and the cap outside it (farming::humidity::GuiView); "" = none.
    pub humidity: String,
}
