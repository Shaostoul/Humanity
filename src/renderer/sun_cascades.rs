//! Near sun cascades: the home and the planet build sites as sun casters.
//!
//! Plan of record: docs/design/sun-cascades.md. Increment 0 is only the
//! fixtures (this file's showcase pin), increment 1 the atlas and the first
//! near cascade.

/// The renderer's sun-cascade state.
#[derive(Default)]
pub struct SunCascades {
    /// Showcase pin over Settings > Planets "Sun shadows" (`sun_shadows`
    /// "0" / "1" / "auto"): `None` follows the setting. A pin rather than a
    /// write to the setting, so a rig A/B never changes what the player's
    /// own config says and "auto" can always put it back.
    pub shadows_pin: Option<bool>,
}

impl SunCascades {
    /// Whether the sun shadow maps render this frame: the setting unless a
    /// showcase pin overrides it.
    pub fn shadows_on(&self, setting: bool) -> bool {
        self.shadows_pin.unwrap_or(setting)
    }
}
