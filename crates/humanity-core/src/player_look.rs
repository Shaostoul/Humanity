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

/// One group of face and body detail sliders the character page shows (2026-11-09, "the look";
/// `data/characters/details.ron`, published by the engine), under `characters.detail_group_<group>`.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailBlock {
    pub group: String,
    pub sliders: Vec<DetailSliderView>,
}

/// One detail slider as the page draws it: its id (its words `characters.detail_<id>` and its value
/// in the look), its ends' word ids (`characters.detail_end_<end>`), and whether it has one end
/// only (0 to 1) rather than two (-1 to 1, the body as made at 0).
#[derive(Debug, Clone, PartialEq)]
pub struct DetailSliderView {
    pub id: String,
    pub low: String,
    pub high: String,
    pub single: bool,
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

/// The most face and body detail values a look carries (2026-11-09, "the look": the sliders a
/// person moved off the body as made; a stranger's look never carries a wall of them).
pub const DETAIL_MAX: usize = 128;
/// The most outfit slots a look names.
pub const OUTFIT_MAX: usize = 8;
/// A detail value's step on the wire: a hundredth, finer than a slider is set and short to send.
pub const DETAIL_STEP: f32 = 0.01;

/// What a look says the person wears, by id (2026-11-09, "the look"): the hair, brows and lashes
/// (garment ids of `data/characters/garments.ron`; "" is the viewer's default for them, "none" wears
/// none) and each outfit slot's cosmetic (`data/cosmetics/cosmetics.csv`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LookWear {
    pub hair: String,
    pub brows: String,
    pub lashes: String,
    pub outfit: BTreeMap<String, String>,
}

impl LookWear {
    fn is_empty(&self) -> bool {
        self.hair.is_empty() && self.brows.is_empty() && self.lashes.is_empty() && self.outfit.is_empty()
    }
}

/// The ids a look may name: the shipped detail sliders (with which run one way only, 0 to 1), the
/// garments and the cosmetics and their slots. A look naming anything else has it dropped by the
/// reader, at the relay and in the app alike (an unknown id is never passed on or trusted).
#[derive(Debug, Clone, Default)]
pub struct LookIds {
    /// Detail slider id to "has one end only".
    pub detail: std::collections::HashMap<String, bool>,
    pub garments: std::collections::HashSet<String>,
    /// Cosmetic id to its slot.
    pub cosmetics: std::collections::HashMap<String, String>,
}

impl LookIds {
    /// From the texts of `data/characters/details.ron`, `data/characters/garments.ron` and
    /// `data/cosmetics/cosmetics.csv` (a file that does not read gives no ids of its kind).
    pub fn from_texts(details_ron: &str, garments_ron: &str, cosmetics_csv: &str) -> Self {
        #[derive(serde::Deserialize)]
        struct Slider {
            id: String,
            low: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct Details {
            sliders: Vec<Slider>,
        }
        #[derive(serde::Deserialize)]
        struct Garment {
            id: String,
        }
        #[derive(serde::Deserialize)]
        struct Garments {
            garments: Vec<Garment>,
        }
        let detail = ron::from_str::<Details>(details_ron).map(|d| d.sliders.into_iter().map(|s| (s.id, s.low.is_empty())).collect()).unwrap_or_default();
        let garments = ron::from_str::<Garments>(garments_ron).map(|g| g.garments.into_iter().map(|x| x.id).collect()).unwrap_or_default();
        let cosmetics = cosmetics_csv
            .lines()
            .skip(1)
            .filter_map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (f.len() >= 3).then(|| (f[0].trim().to_string(), f[2].trim().to_string()))
            })
            .collect();
        LookIds { detail, garments, cosmetics }
    }

    /// The shipped ids, compiled in (the relay reads no character files; a modded data folder still
    /// draws its own ids in its own app, through the app's library).
    pub fn shipped() -> &'static LookIds {
        static IDS: std::sync::OnceLock<LookIds> = std::sync::OnceLock::new();
        IDS.get_or_init(|| {
            LookIds::from_texts(
                include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/details.ron")),
                include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/garments.ron")),
                include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/cosmetics/cosmetics.csv")),
            )
        })
    }
}

/// Detail values as a look carries them: only known slider ids, each value finite, clamped to its
/// slider's range (-1 to 1, or 0 to 1 for a one-ended slider), to a hundredth, nonzero, at most
/// `DETAIL_MAX` of them (the first in id order).
pub fn clean_detail<'a>(values: impl IntoIterator<Item = (&'a str, f32)>, ids: &LookIds) -> BTreeMap<String, f32> {
    let mut out = BTreeMap::new();
    for (k, v) in values {
        let Some(&single) = ids.detail.get(k) else { continue };
        if !v.is_finite() {
            continue;
        }
        let lo = if single { 0.0 } else { -1.0 };
        let q = (v.clamp(lo, 1.0) / DETAIL_STEP).round() * DETAIL_STEP;
        if q != 0.0 {
            out.insert(k.to_string(), q);
        }
    }
    while out.len() > DETAIL_MAX {
        let last = out.keys().next_back().cloned().expect("longer than the most, so not empty");
        out.remove(&last);
    }
    out
}

/// What a look says is worn, cleaned: a garment id the shipped garments do not have (or "none") is
/// dropped to "" (the viewer's default), an outfit slot and its cosmetic kept only when the
/// cosmetic is known and is that slot's, at most `OUTFIT_MAX` slots.
pub fn clean_wear(hair: &str, brows: &str, lashes: &str, outfit: impl IntoIterator<Item = (String, String)>, ids: &LookIds) -> LookWear {
    let garment = |id: &str| if id == "none" || ids.garments.contains(id) { id.to_string() } else { String::new() };
    let outfit = outfit
        .into_iter()
        .filter(|(slot, id)| ids.cosmetics.get(id).is_some_and(|s| s == slot))
        .take(OUTFIT_MAX)
        .collect();
    LookWear { hair: garment(hair), brows: garment(brows), lashes: garment(lashes), outfit }
}

#[derive(Debug, Clone, PartialEq, Default)]
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
    /// Eye colour, linear RGB 0 to 1 (2026-11-09; None in an older app's look: the viewer's default).
    pub eyes: Option<[f32; 3]>,
    /// The face and body detail sliders (`Appearance::detail`, 2026-11-09): only those moved off
    /// the body as made, cleaned by `clean_detail`.
    pub detail: BTreeMap<String, f32>,
    /// What they wear, by id (2026-11-09), cleaned by `clean_wear`.
    pub wear: LookWear,
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
        Self::from_json_with(v, LookIds::shipped())
    }

    /// `from_json` against these ids (the tests' own; the game reads with the shipped ones). THE
    /// ONE READER of a look, at the relay and in the app: everything is clamped or dropped here.
    pub fn from_json_with(v: &Value, ids: &LookIds) -> Option<Self> {
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
        // The look (2026-11-09): the eyes, the detail sliders and what is worn, each optional and
        // each cleaned against the known ids (only so many entries are even looked at).
        let eyes = colour(v.get("eyes"));
        let detail = v
            .get("detail")
            .and_then(|s| s.as_object())
            .map(|o| clean_detail(o.iter().take(DETAIL_MAX * 4).filter_map(|(k, x)| Some((k.as_str(), x.as_f64()? as f32))), ids))
            .unwrap_or_default();
        let wear = v.get("wear").map_or_else(LookWear::default, |w| {
            let s = |k: &str| w.get(k).and_then(|x| x.as_str()).unwrap_or("");
            let outfit = w
                .get("outfit")
                .and_then(|o| o.as_object())
                .map(|o| o.iter().take(OUTFIT_MAX * 4).filter_map(|(slot, id)| Some((slot.clone(), id.as_str()?.to_string()))).collect::<Vec<_>>())
                .unwrap_or_default();
            clean_wear(s("hair"), s("brows"), s("lashes"), outfit, ids)
        });
        Some(Self { skin, hair, height: h.clamp(HEIGHT_MIN, HEIGHT_MAX), body, shape, eyes, detail, wear })
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({ "skin": self.skin, "hair": self.hair, "height": self.height, "body": self.body });
        if !self.shape.is_empty() {
            // As short decimals (0.62, not the 0.6200000047683716 an f32 widens to).
            let shape: serde_json::Map<String, Value> = self.shape.iter().map(|(k, x)| (k.clone(), json!(((*x as f64) * 1000.0).round() / 1000.0))).collect();
            v["shape"] = Value::Object(shape);
        }
        // The look's parts, each left out when empty, so a look without them is the same bytes as
        // before.
        if let Some(e) = self.eyes {
            v["eyes"] = json!(e);
        }
        if !self.detail.is_empty() {
            let detail: serde_json::Map<String, Value> = self.detail.iter().map(|(k, x)| (k.clone(), json!(((*x as f64) * 100.0).round() / 100.0))).collect();
            v["detail"] = Value::Object(detail);
        }
        if !self.wear.is_empty() {
            let mut w = serde_json::Map::new();
            for (k, x) in [("hair", &self.wear.hair), ("brows", &self.wear.brows), ("lashes", &self.wear.lashes)] {
                if !x.is_empty() {
                    w.insert(k.to_string(), json!(x));
                }
            }
            if !self.wear.outfit.is_empty() {
                w.insert("outfit".to_string(), json!(self.wear.outfit));
            }
            v["wear"] = Value::Object(w);
        }
        v
    }

    /// The parts of a character's appearance a remote figure shows (with what they wear in each
    /// outfit slot), through the same cleaning a received look gets.
    pub fn from_appearance(a: &crate::ecs::components::Appearance) -> Self {
        Self::from_appearance_wearing(a, &Default::default())
    }

    /// `from_appearance` with the outfit they wear (slot to cosmetic id).
    pub fn from_appearance_wearing(a: &crate::ecs::components::Appearance, outfit: &std::collections::HashMap<String, String>) -> Self {
        let ids = LookIds::shipped();
        Self {
            skin: a.skin_tone.map(|c| c.clamp(0.0, 1.0)),
            hair: a.hair_color.map(|c| c.clamp(0.0, 1.0)),
            height: a.height_scale.clamp(HEIGHT_MIN, HEIGHT_MAX),
            body: clean_body_id(&a.body_type),
            shape: clean_shape(a.shape.iter().map(|(k, v)| (k.as_str(), *v))),
            eyes: Some(a.eye_color.map(|c| c.clamp(0.0, 1.0))),
            detail: clean_detail(a.detail.iter().map(|(k, v)| (k.as_str(), *v)), ids),
            wear: clean_wear(&a.hair_style, &a.brows, &a.lashes, outfit.iter().map(|(k, v)| (k.clone(), v.clone())), ids),
        }
    }
}

/// THE LOOK'S RATE (2026-11-09; the operator asked what it would take): a changed look goes out
/// when Done is pressed, at most once every `LOOK_EVERY_S` seconds per player, collapsed to the
/// latest (the app holds a newer one until the wait is over; the relay refuses one sooner). Both
/// ends keep the rule here.
pub const LOOK_EVERY_S: f64 = 10.0;

/// May a look go out (or be accepted) `now` seconds, given the last one went at `last`?
pub fn look_due(last: Option<f64>, now: f64) -> bool {
    last.map_or(true, |t| now - t >= LOOK_EVERY_S || now < t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A LOOK IS READ SAFELY (2026-09-29). A good one round-trips; a
    /// stranger's out-of-range numbers are clamped; a malformed one is
    /// refused. Red check, run: dropping the clamps fails the second assertion.
    #[test]
    fn a_look_is_read_safely() {
        let good = PlayerLook { skin: [0.8, 0.6, 0.5], hair: [0.2, 0.1, 0.05], height: 1.1, body: "female".into(), shape: BTreeMap::new(), ..Default::default() };
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
        let good = PlayerLook { skin: [0.5, 0.4, 0.3], hair: [0.1, 0.1, 0.1], height: 1.0, body: "male".into(), shape, ..Default::default() };
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

    /// THE LOOK'S ONE READER (2026-11-09, "the look"): a look's detail sliders and what it wears go
    /// through `from_json` at the relay and in the app alike. A known slider's value round-trips to
    /// a hundredth; an UNKNOWN slider id is dropped; an OUT-OF-RANGE value is clamped to its
    /// slider's range (a one-ended slider never goes below 0); a value that is not a number, and a
    /// zero, are left out; TOO MANY values are cut to `DETAIL_MAX`; a garment or cosmetic nothing
    /// knows reads as the default, and a cosmetic in another slot's place is dropped. An old look
    /// without any of it is the same bytes as before.
    /// Seen red 2026-11-09 with `clean_detail` keeping ids it does not know (each read as a
    /// two-ended slider): "assertion failed: !l.detail.contains_key(\"chin_width\") &&
    /// !l.detail.contains_key(\"ear_scale\") && !l.detail.contains_key(\"../evil\")"; the relay's
    /// own test, reading through the same reader, failed the same day on "clamped, the unknown
    /// slider dropped": left {"../bad": 0.5, "nose_hump": 1.0}.
    #[test]
    fn a_looks_details_and_wear_are_read_through_the_one_clamping_reader() {
        let ids = LookIds::shipped();
        assert!(ids.detail.len() > 200 && ids.detail.get("nose_hump") == Some(&false) && ids.detail.get("head_oval") == Some(&true), "the shipped sliders read");
        assert!(ids.garments.contains("bob01") && ids.cosmetics.get("mh_shoes01").map(String::as_str) == Some("feet"));
        let mut detail = BTreeMap::new();
        detail.insert("nose_hump".to_string(), 0.37);
        let good = PlayerLook {
            skin: [0.5, 0.4, 0.3],
            hair: [0.1, 0.1, 0.1],
            height: 1.0,
            body: "female".into(),
            eyes: Some([0.2, 0.3, 0.4]),
            detail,
            wear: LookWear { hair: "bob01".into(), brows: "none".into(), lashes: String::new(), outfit: [("feet".to_string(), "mh_shoes01".to_string())].into_iter().collect() },
            ..Default::default()
        };
        assert_eq!(PlayerLook::from_json(&good.to_json()), Some(good.clone()), "a good look round-trips");
        // A stranger's.
        let mut wild = good.to_json();
        let mut many = serde_json::Map::new();
        many.insert("nose_hump".into(), json!(9.0));
        many.insert("head_oval".into(), json!(-0.7));
        many.insert("chin_width".into(), json!("lots"));
        many.insert("ear_scale".into(), json!(0.0));
        many.insert("../evil".into(), json!(0.5));
        wild["detail"] = Value::Object(many);
        wild["wear"] = json!({"hair": "../../hair", "brows": "eyebrow001", "outfit": {"feet": "mh_female_casualsuit01", "chest": "no_such_thing", "head": "mh_fedora01"}});
        let l = PlayerLook::from_json(&wild).unwrap();
        assert_eq!(l.detail.get("nose_hump").copied(), Some(1.0), "clamped to the slider's high end");
        assert_eq!(l.detail.get("head_oval").copied(), None, "a one-ended slider below 0 is at 0, which is left out");
        assert!(!l.detail.contains_key("chin_width") && !l.detail.contains_key("ear_scale") && !l.detail.contains_key("../evil"));
        assert_eq!(l.detail.len(), 1, "only the known, numeric, nonzero value is kept: {:?}", l.detail);
        // Too many: every known slider set, cut to the most a look carries.
        let mut all = good.to_json();
        all["detail"] = Value::Object(ids.detail.keys().map(|k| (k.clone(), json!(0.25))).collect());
        assert!(ids.detail.len() > DETAIL_MAX);
        assert_eq!(PlayerLook::from_json(&all).unwrap().detail.len(), DETAIL_MAX, "too many are cut to the most a look carries");
        assert_eq!(l.wear.hair, "", "an unknown hair reads as the default");
        assert_eq!(l.wear.brows, "eyebrow001");
        assert_eq!(l.wear.outfit.len(), 1, "only the known cosmetic in its own slot is kept: {:?}", l.wear.outfit);
        assert_eq!(l.wear.outfit.get("head").map(String::as_str), Some("mh_fedora01"));
        // An old look: nothing new on the wire, nothing new read.
        let old = json!({"skin": [0.5, 0.4, 0.3], "hair": [0.1, 0.1, 0.1], "height": 1.0, "body": "male"});
        let read = PlayerLook::from_json(&old).unwrap();
        assert!(read.detail.is_empty() && read.wear == LookWear::default() && read.eyes.is_none());
        let back = read.to_json();
        assert!(back.get("detail").is_none() && back.get("wear").is_none() && back.get("eyes").is_none(), "an old look gains nothing on the wire: {back}");
    }

    /// THE LOOK'S RATE: once every LOOK_EVERY_S seconds, the first at once; a clock that went back
    /// (a restarted relay's) never blocks one.
    #[test]
    fn a_look_goes_out_at_most_once_every_ten_seconds() {
        assert!(look_due(None, 5.0));
        assert!(!look_due(Some(100.0), 104.0));
        assert!(look_due(Some(100.0), 110.0));
        assert!(look_due(Some(100.0), 50.0));
    }
}
