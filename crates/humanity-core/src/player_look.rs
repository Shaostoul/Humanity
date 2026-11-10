//! How another player looks, in a shared world (2026-09-29, Tier D).
//!
//! The parts of a character a remote figure can show: skin tone, hair colour
//! and height. The desktop app sends its own in `game_join`; the relay keeps
//! it on the player's entity (so a later joiner's snapshot carries it) and
//! passes it on in `game_player_joined`; each client keeps it on the
//! `RemotePlayer`. Both the app and the relay compile this, so both read the
//! same shape, and both clamp what they receive: a stranger's data never
//! makes a giant or an impossible colour.

use std::collections::BTreeMap;

use serde_json::{json, Value};

/// The height multiplier's range (1.0 is about 1.7 m), the same the character
/// creator allows.
pub const HEIGHT_MIN: f32 = 0.8;
pub const HEIGHT_MAX: f32 = 1.25;

/// The most shape slider values a look carries (2026-11-09: the MakeHuman bodies
/// have six, muscle, weight, proportions and three ancestry shares; room to grow,
/// and a stranger's look never carries a wall of them).
pub const SHAPE_MAX: usize = 16;
/// The longest slider id a look carries.
pub const SHAPE_ID_MAX: usize = 24;

/// A slider id as a look carries it: lower case letters, digits, `_` and `-`, at
/// most `SHAPE_ID_MAX`; anything else is "".
pub fn clean_shape_id(id: &str) -> String {
    let ok = !id.is_empty() && id.len() <= SHAPE_ID_MAX && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        id.to_string()
    } else {
        String::new()
    }
}

/// Shape slider values as a look carries them: plain ids, each value finite and
/// clamped to 0 to 1, at most `SHAPE_MAX` of them (the first in id order).
pub fn clean_shape<'a>(values: impl IntoIterator<Item = (&'a str, f32)>) -> BTreeMap<String, f32> {
    let mut out = BTreeMap::new();
    for (k, v) in values {
        let id = clean_shape_id(k);
        if id.is_empty() || !v.is_finite() {
            continue;
        }
        // To a thousandth: finer than a slider can be set, and short on the wire.
        out.insert(id, (v.clamp(0.0, 1.0) * 1000.0).round() / 1000.0);
    }
    while out.len() > SHAPE_MAX {
        let last = out.keys().next_back().cloned().expect("longer than the most, so not empty");
        out.remove(&last);
    }
    out
}

/// The middle of a shape slider that runs from one end to the other (build, proportions): the body
/// as made. A look without a value for such a slider has it here.
pub const SHAPE_MIDDLE: f32 = 0.5;

/// A run of shape sliders the character page shows together (one row of a body file's
/// `shape.sliders`, data/characters/bodies/*.ron; humanity-animation reads the files and the
/// engine publishes these for the page). Here in the core so the page needs nothing of the
/// animation crate.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderBlock {
    /// Its heading (`characters.shape_group_<group>`).
    pub group: String,
    /// Each slider's id, which names its words (`characters.shape_<id>`) and its value in the look.
    pub sliders: Vec<String>,
    /// The sliders are shares of one whole (a blend): moving one moves the others so they still add
    /// up to one, and each shows its share. Otherwise each runs from its low end to its high end
    /// (`characters.shape_<id>_low`, `_high`) with the body as made in the middle.
    pub shares: bool,
}

/// A blend's shares from its parts' values: the ones given, an even share for the ones not, made to
/// add up to one (all nothing reads as the even mix).
pub fn blend_shares(raw: &[Option<f32>]) -> Vec<f32> {
    let n = raw.len().max(1) as f32;
    let vals: Vec<f32> = raw.iter().map(|v| v.filter(|x| x.is_finite()).map_or(1.0 / n, |x| x.max(0.0))).collect();
    let sum: f32 = vals.iter().sum();
    if sum > 1e-6 {
        vals.iter().map(|v| v / sum).collect()
    } else {
        vec![1.0 / n; raw.len()]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerLook {
    /// Skin tone, linear RGB 0 to 1.
    pub skin: [f32; 3],
    /// Hair colour, linear RGB 0 to 1.
    pub hair: [f32; 3],
    /// Height multiplier, HEIGHT_MIN to HEIGHT_MAX.
    pub height: f32,
    /// The body they chose (`Appearance::body_type`, an id of data/characters/characters.ron's
    /// bodies: "female", "male"; 2026-11-09), so others see the body they picked. At most
    /// `BODY_ID_MAX` characters of a-z, 0-9, `_` and `-`; anything else reads as "" (the viewer's
    /// own default for a player).
    pub body: String,
    /// The body's shape (`Appearance::shape`, 2026-11-09): each character page slider's value by
    /// its id (build, proportions, ancestry shares; the ids are the body files' sliders), 0 to 1.
    /// A slider not here is at its middle, the body as made; so a look without any (an older app's)
    /// shows the body as made. Cleaned by `clean_shape`.
    pub shape: BTreeMap<String, f32>,
}

/// The longest body id a look carries.
pub const BODY_ID_MAX: usize = 32;

/// A body id as a look carries it: lower case letters, digits, `_` and `-`, at most `BODY_ID_MAX`;
/// anything else is "" (a stranger's data never names a file path or a wall of text).
pub fn clean_body_id(id: &str) -> String {
    let ok = !id.is_empty() && id.len() <= BODY_ID_MAX && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        id.to_string()
    } else {
        String::new()
    }
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
        // The body is optional: a look without one (or with a bad one) shows the viewer's default.
        let body = v.get("body").and_then(|b| b.as_str()).map(clean_body_id).unwrap_or_default();
        // So is the shape: `{"muscle": 0.7, ...}`; a look without it (an older app's) is the body
        // as made, and a value that is not a number is left out.
        let shape = v
            .get("shape")
            .and_then(|s| s.as_object())
            .map(|o| clean_shape(o.iter().take(SHAPE_MAX * 4).filter_map(|(k, x)| Some((k.as_str(), x.as_f64()? as f32)))))
            .unwrap_or_default();
        Some(Self { skin, hair, height: h.clamp(HEIGHT_MIN, HEIGHT_MAX), body, shape })
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({ "skin": self.skin, "hair": self.hair, "height": self.height, "body": self.body });
        if !self.shape.is_empty() {
            // As short decimals (0.62, not the 0.6200000047683716 an f32 widens to).
            let shape: serde_json::Map<String, Value> = self.shape.iter().map(|(k, x)| (k.clone(), json!(((*x as f64) * 1000.0).round() / 1000.0))).collect();
            v["shape"] = Value::Object(shape);
        }
        v
    }

    /// The parts of a character's appearance a remote figure shows.
    pub fn from_appearance(a: &crate::ecs::components::Appearance) -> Self {
        Self {
            skin: a.skin_tone.map(|c| c.clamp(0.0, 1.0)),
            hair: a.hair_color.map(|c| c.clamp(0.0, 1.0)),
            height: a.height_scale.clamp(HEIGHT_MIN, HEIGHT_MAX),
            body: clean_body_id(&a.body_type),
            shape: clean_shape(a.shape.iter().map(|(k, v)| (k.as_str(), *v))),
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
        let good = PlayerLook { skin: [0.8, 0.6, 0.5], hair: [0.2, 0.1, 0.05], height: 1.1, body: "female".into(), shape: BTreeMap::new() };
        assert_eq!(PlayerLook::from_json(&good.to_json()), Some(good));
        let wild = json!({"skin": [2.0, -1.0, 0.5], "hair": [0.1, 0.1, 0.1], "height": 50.0});
        let l = PlayerLook::from_json(&wild).unwrap();
        assert_eq!((l.skin, l.height), ([1.0, 0.0, 0.5], HEIGHT_MAX));
        assert_eq!(PlayerLook::from_json(&json!({"skin": [1.0, 1.0], "hair": [0.0, 0.0, 0.0], "height": 1.0})), None);
        assert_eq!(PlayerLook::from_json(&json!({"hair": [0.0, 0.0, 0.0], "height": 1.0})), None);
    }

    /// THE BODY A LOOK CARRIES (2026-11-09) is a plain id or nothing: a good one round-trips, a
    /// look without one still reads (the viewer's default body), and a stranger's path, upper case,
    /// spaces or wall of text read as "". Red check, run 2026-11-09: `clean_body_id` returning its
    /// input unchecked fails on "../../etc/passwd".
    #[test]
    fn a_looks_body_is_a_plain_id_or_nothing() {
        let v = json!({"skin": [0.5, 0.4, 0.3], "hair": [0.1, 0.1, 0.1], "height": 1.0, "body": "male"});
        assert_eq!(PlayerLook::from_json(&v).unwrap().body, "male");
        let none = json!({"skin": [0.5, 0.4, 0.3], "hair": [0.1, 0.1, 0.1], "height": 1.0});
        assert_eq!(PlayerLook::from_json(&none).unwrap().body, "");
        for bad in ["../../etc/passwd", "Male", "a body", &"x".repeat(BODY_ID_MAX + 1), ""] {
            assert_eq!(clean_body_id(bad), "", "{bad}");
        }
        assert_eq!(clean_body_id("makehuman_female-2"), "makehuman_female-2");
        let a = crate::ecs::components::Appearance { body_type: "female".into(), ..Default::default() };
        assert_eq!(PlayerLook::from_appearance(&a).body, "female");
    }

    /// THE SHAPE A LOOK CARRIES (2026-11-09, the character page's build and ancestry sliders): a
    /// look's slider values round-trip as short decimals; a look WITHOUT a shape (an older app's,
    /// or one that never moved a slider) reads with none, which every viewer draws as the body as
    /// made; a stranger's values are clamped to 0 to 1, a value that is not a number and an id that
    /// is not a plain one are left out, and no look carries more than SHAPE_MAX of them. The
    /// Appearance's shape is what goes out.
    /// Seen red 2026-11-09 with `clean_shape` inserting its values unclamped: "left: Some(7.0)
    /// right: Some(1.0)" on the stranger's muscle.
    #[test]
    fn a_looks_shape_is_clamped_and_an_old_look_reads_as_the_body_as_made() {
        let mut shape = BTreeMap::new();
        shape.insert("muscle".to_string(), 0.62);
        shape.insert("african".to_string(), 0.25);
        let good = PlayerLook { skin: [0.5, 0.4, 0.3], hair: [0.1, 0.1, 0.1], height: 1.0, body: "male".into(), shape };
        let wire = good.to_json();
        assert_eq!(wire["shape"].to_string(), r#"{"african":0.25,"muscle":0.62}"#, "short decimals on the wire");
        assert_eq!(PlayerLook::from_json(&wire), Some(good));
        // An old look: no shape at all, and nothing extra on the wire for one that has none.
        let old = json!({"skin": [0.5, 0.4, 0.3], "hair": [0.1, 0.1, 0.1], "height": 1.0, "body": "female"});
        let read = PlayerLook::from_json(&old).unwrap();
        assert!(read.shape.is_empty());
        assert!(read.to_json().get("shape").is_none());
        // A stranger's.
        let mut wild = old.clone();
        let mut many = serde_json::Map::new();
        many.insert("muscle".into(), json!(7.0));
        many.insert("weight".into(), json!(-3.0));
        many.insert("asian".into(), json!("lots"));
        many.insert("../x".into(), json!(0.5));
        many.insert("Weight".into(), json!(0.5));
        for i in 0..40 {
            many.insert(format!("z{i:02}"), json!(0.5));
        }
        wild["shape"] = Value::Object(many);
        let l = PlayerLook::from_json(&wild).unwrap();
        assert_eq!(l.shape.get("muscle").copied(), Some(1.0));
        assert_eq!(l.shape.get("weight").copied(), Some(0.0));
        assert!(!l.shape.contains_key("asian") && !l.shape.contains_key("../x") && !l.shape.contains_key("Weight"));
        assert_eq!(l.shape.len(), SHAPE_MAX);
        assert_eq!(clean_shape_id(&"x".repeat(SHAPE_ID_MAX + 1)), "");
        // From the Appearance.
        let mut a = crate::ecs::components::Appearance::default();
        a.shape.insert("weight".into(), 0.8);
        a.shape.insert("bad id".into(), 0.8);
        assert_eq!(PlayerLook::from_appearance(&a).shape.into_iter().collect::<Vec<_>>(), vec![("weight".to_string(), 0.8)]);
    }
}
