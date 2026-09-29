//! How another player looks, in a shared world (2026-09-29, Tier D).
//!
//! The parts of a character a remote figure can show: skin tone, hair colour
//! and height. The desktop app sends its own in `game_join`; the relay keeps
//! it on the player's entity (so a later joiner's snapshot carries it) and
//! passes it on in `game_player_joined`; each client keeps it on the
//! `RemotePlayer`. Both the app and the relay compile this, so both read the
//! same shape, and both clamp what they receive: a stranger's data never
//! makes a giant or an impossible colour.

use serde_json::{json, Value};

/// The height multiplier's range (1.0 is about 1.7 m), the same the character
/// creator allows.
pub const HEIGHT_MIN: f32 = 0.8;
pub const HEIGHT_MAX: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerLook {
    /// Skin tone, linear RGB 0 to 1.
    pub skin: [f32; 3],
    /// Hair colour, linear RGB 0 to 1.
    pub hair: [f32; 3],
    /// Height multiplier, HEIGHT_MIN to HEIGHT_MAX.
    pub height: f32,
}

fn colour(v: Option<&Value>) -> Option<[f32; 3]> {
    let a = v?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let mut out = [0.0_f32; 3];
    for (i, c) in a.iter().enumerate() {
        let f = c.as_f64()? as f32;
        if !f.is_finite() {
            return None;
        }
        out[i] = f.clamp(0.0, 1.0);
    }
    Some(out)
}

impl PlayerLook {
    /// From an `appearance` JSON object: `{"skin":[r,g,b],"hair":[r,g,b],
    /// "height":h}`. None when it is absent or malformed; colours and height
    /// are clamped to their ranges.
    pub fn from_json(v: &Value) -> Option<Self> {
        let skin = colour(v.get("skin"))?;
        let hair = colour(v.get("hair"))?;
        let h = v.get("height").and_then(|x| x.as_f64())? as f32;
        if !h.is_finite() {
            return None;
        }
        Some(Self { skin, hair, height: h.clamp(HEIGHT_MIN, HEIGHT_MAX) })
    }

    pub fn to_json(&self) -> Value {
        json!({ "skin": self.skin, "hair": self.hair, "height": self.height })
    }

    /// The parts of a character's appearance a remote figure shows.
    pub fn from_appearance(a: &crate::ecs::components::Appearance) -> Self {
        Self {
            skin: a.skin_tone.map(|c| c.clamp(0.0, 1.0)),
            hair: a.hair_color.map(|c| c.clamp(0.0, 1.0)),
            height: a.height_scale.clamp(HEIGHT_MIN, HEIGHT_MAX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A LOOK IS READ SAFELY (2026-09-29). A good one round-trips; a
    /// stranger's out-of-range numbers are clamped; a malformed one is
    /// refused. Red check, run: dropping the clamps fails the second assertion.
    #[test]
    fn a_look_is_read_safely() {
        let good = PlayerLook { skin: [0.8, 0.6, 0.5], hair: [0.2, 0.1, 0.05], height: 1.1 };
        assert_eq!(PlayerLook::from_json(&good.to_json()), Some(good));
        let wild = json!({"skin": [2.0, -1.0, 0.5], "hair": [0.1, 0.1, 0.1], "height": 50.0});
        let l = PlayerLook::from_json(&wild).unwrap();
        assert_eq!((l.skin, l.height), ([1.0, 0.0, 0.5], HEIGHT_MAX));
        assert_eq!(PlayerLook::from_json(&json!({"skin": [1.0, 1.0], "hair": [0.0, 0.0, 0.0], "height": 1.0})), None);
        assert_eq!(PlayerLook::from_json(&json!({"hair": [0.0, 0.0, 0.0], "height": 1.0})), None);
    }
}
