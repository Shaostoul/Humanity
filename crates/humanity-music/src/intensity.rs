//! Danger and action: the score rising with the game's intensity (docs/design/music.md, section 7b).
//!
//! The operator, 2026-11-06: "Let's also see what we can generate for high octane situations like a
//! meteor storm puncturing a bunch of holes through the ship or an alien invasion." The game sets an
//! INTENSITY, 0..1 (a hull breach, a meteor storm, an invasion, combat; today the Dev page's slider and
//! the `music_intensity` showcase verb drive it). `data/music/intensity.ron` cuts it into BANDS, each
//! naming the mood it plays (none: the place's own mood, so calm is wherever you are):
//!
//! - **Rising** into a higher band: on the next beat a STINGER (a big low drum, an alarm in the bells,
//!   data), a RISER (a noise sweep) building to the next bar line, and at that bar line the danger mood
//!   begins a new phrase. Never a cut: the change waits for a beat, then a bar.
//! - **Falling** back: at the next `fall_bars` boundary (2 bars by default), with no stinger; the
//!   long pad envelopes carry the settle.
//! - A `hysteresis` keeps an intensity hovering at a band's edge from flipping it every bar.
//!
//! Inside a band, the mood's own data does the rest: its tempo can follow the intensity
//! (`tempo_follows_intensity`), its roles and drum parts join at their `min_intensity` (taiko
//! ostinatos double at the peak, the brass stabs arrive), and an URGENT mood plays as written
//! whatever the person's genre (danger sounds like danger in every genre; the genre's tape colour
//! still applies).

use serde::{Deserialize, Serialize};

/// One band.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Band {
    /// Where it starts, 0..1.
    pub from: f32,
    /// The mood it plays (none: the place's own).
    #[serde(default)]
    pub mood: Option<String>,
}

/// A note of the stinger or the riser.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gesture {
    pub instrument: String,
    pub note: String,
    #[serde(default = "one")]
    pub velocity: f32,
    /// When, in beats after the gesture begins.
    #[serde(default)]
    pub beat: f32,
}

fn one() -> f32 {
    1.0
}

/// data/music/intensity.ron.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntensityMap {
    pub bands: Vec<Band>,
    #[serde(default = "default_hysteresis")]
    pub hysteresis: f32,
    /// The notes played on the beat after a rise.
    #[serde(default)]
    pub stinger: Vec<Gesture>,
    /// The sweep that builds to the bar line where the danger mood begins.
    #[serde(default)]
    pub riser: Option<Gesture>,
    /// Falling back waits for a boundary of this many bars.
    #[serde(default = "default_fall_bars")]
    pub fall_bars: u32,
}

fn default_hysteresis() -> f32 {
    0.08
}
fn default_fall_bars() -> u32 {
    2
}

impl Default for IntensityMap {
    fn default() -> Self {
        IntensityMap { bands: vec![Band { from: 0.0, mood: None }], hysteresis: 0.08, stinger: Vec::new(), riser: None, fall_bars: 2 }
    }
}

/// The band an intensity is in, given the band it is in now (the hysteresis holds it there until
/// the intensity is clearly past an edge). `froms` are the bands' starts, ascending.
pub fn band_for(froms: &[f32], current: usize, intensity: f32, hysteresis: f32) -> usize {
    if froms.is_empty() {
        return 0;
    }
    let mut b = current.min(froms.len() - 1);
    // Up while past the next band's start by the hysteresis.
    while b + 1 < froms.len() && intensity >= froms[b + 1] + hysteresis * 0.5 {
        b += 1;
    }
    // Down while under this band's start by the hysteresis.
    while b > 0 && intensity < froms[b] - hysteresis * 0.5 {
        b -= 1;
    }
    b
}
