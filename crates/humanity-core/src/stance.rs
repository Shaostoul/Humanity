//! HOW A PERSON STANDS, AS THE NETWORK CARRIES IT (character animation rung 2a, 2026-11-09;
//! docs/design/character-animation.md section 8.5, "What crosses the network").
//!
//! Poses never cross the network: each client animates every body it draws from its interpolated
//! positions. But a crouch cannot be read from positions, and neither, reliably, can a jump (a
//! position rising may be a stair). So the 15-a-second movement message (`game_position_update`)
//! carries one small number beside the position: the person's STANCE and whether they are OFF THE
//! GROUND. Plan 8.5's budget: "a stance byte (stand, crouch, sit, climb, swim, fly, zero-g)".
//!
//! THE WIRE: `"stance": n`, a whole number, the stance's code plus 128 when off the ground; LEFT OUT
//! when the person stands on the ground (code 0), so a standing player's message is the same bytes
//! as before this rung, and a message without it (an older app's) reads as standing. Both the
//! relay (which passes it on to the players who have them in view) and the app read it through
//! [`Posture::from_json`], which clamps: a number that is not one of the codes, or not a whole
//! number from 0 to 255, reads as standing on the ground.
//!
//! Standing and crouching are sent since rung 2a, sitting (and lying) and climbing since rung 2b
//! (2026-11-10, with the act that says on what); swimming, flight and zero-g wait for the rungs that
//! animate them. The codes are fixed so an app that does not know a newer stance still reads the
//! number and draws the person standing.

use serde_json::Value;

/// A person's stance (plan 8.5's list). The code is the wire value; never renumber one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stance {
    #[default]
    Stand,
    Crouch,
    Sit,
    Climb,
    Swim,
    Fly,
    ZeroG,
}

impl Stance {
    /// The wire code.
    pub fn code(self) -> u8 {
        match self {
            Stance::Stand => 0,
            Stance::Crouch => 1,
            Stance::Sit => 2,
            Stance::Climb => 3,
            Stance::Swim => 4,
            Stance::Fly => 5,
            Stance::ZeroG => 6,
        }
    }

    /// The stance of a wire code; None for a code that is not one.
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Stance::Stand,
            1 => Stance::Crouch,
            2 => Stance::Sit,
            3 => Stance::Climb,
            4 => Stance::Swim,
            5 => Stance::Fly,
            6 => Stance::ZeroG,
            _ => return None,
        })
    }
}

/// The off-the-ground flag's bit in the wire number.
pub const AIRBORNE_BIT: u8 = 128;

/// A person's stance and whether they are off the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Posture {
    pub stance: Stance,
    pub airborne: bool,
}

impl Posture {
    /// The wire number: the stance's code, plus [`AIRBORNE_BIT`] when off the ground.
    pub fn to_wire(self) -> u8 {
        self.stance.code() | if self.airborne { AIRBORNE_BIT } else { 0 }
    }

    /// The posture of a wire number, clamped: an unknown stance code reads as standing (and the
    /// flag as written), anything that is not a whole number from 0 to 255 as standing on the ground.
    pub fn from_wire(n: u64) -> Self {
        let Ok(byte) = u8::try_from(n) else { return Posture::default() };
        Posture { stance: Stance::from_code(byte & !AIRBORNE_BIT).unwrap_or_default(), airborne: byte & AIRBORNE_BIT != 0 }
    }

    /// The posture a message's `stance` field says; standing on the ground when it is missing or
    /// not a whole number.
    pub fn from_json(v: Option<&Value>) -> Self {
        v.and_then(Value::as_u64).map(Posture::from_wire).unwrap_or_default()
    }

    /// The `stance` field to put in a message, or None when standing on the ground (the field is
    /// then left out, so the message is the same bytes as before this rung).
    pub fn to_json(self) -> Option<Value> {
        (self != Posture::default()).then(|| Value::from(self.to_wire()))
    }
}

// ── Where a person looks and what they are using (character animation rung 2b, 2026-11-10) ──
//
// Plan 8.5 adds "the look pitch in a byte (yaw already goes), and, only when they change, the
// interaction (anchor id, ...)". Both ride the same movement message and are read the same way at
// both ends: one clamping reader each, left out of the message when there is nothing to say (looking
// level; using nothing), so an idle player's message is the same bytes as before.

/// The longest anchor id the network carries (and data/characters/interactions.ron may hold).
pub const ANCHOR_ID_MAX: usize = 32;

/// The most a pitch may say, degrees (looking straight up or down).
pub const PITCH_MAX_DEG: i64 = 89;

/// The look's pitch on the wire: whole degrees, up positive, clamped to +-`PITCH_MAX_DEG`; None
/// (the field left out) when it rounds to level.
pub fn pitch_to_json(pitch_rad: f32) -> Option<Value> {
    if !pitch_rad.is_finite() {
        return None;
    }
    let deg = (pitch_rad.to_degrees().round() as i64).clamp(-PITCH_MAX_DEG, PITCH_MAX_DEG);
    (deg != 0).then(|| Value::from(deg))
}

/// The look's pitch a message's `pitch` field says, radians: a whole number of degrees clamped to
/// +-`PITCH_MAX_DEG`; level when it is missing or not a whole number (a fraction, a string, null).
pub fn pitch_from_json(v: Option<&Value>) -> f32 {
    v.and_then(Value::as_i64).map_or(0.0, |d| (d.clamp(-PITCH_MAX_DEG, PITCH_MAX_DEG) as f32).to_radians())
}

/// The anchors the shipped game knows (data/characters/interactions.ron's ids), compiled in: the
/// relay reads no character files, and an id outside these is dropped by both ends, never trusted.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnchorIds {
    pub ids: std::collections::BTreeSet<String>,
}

impl AnchorIds {
    /// The ids in an interactions.ron text: every row's `id`, read without the rest (the rows'
    /// shapes are the animation crate's, which the relay does not compile).
    pub fn from_text(text: &str) -> Self {
        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
        }
        // Each row's other fields are skipped by serde (a struct ignores fields it does not name).
        let ids = ron::from_str::<Vec<Row>>(text).map(|rows| rows.into_iter().map(|r| r.id).collect()).unwrap_or_default();
        AnchorIds { ids }
    }

    /// The shipped ids, compiled in.
    pub fn shipped() -> &'static AnchorIds {
        static IDS: std::sync::OnceLock<AnchorIds> = std::sync::OnceLock::new();
        IDS.get_or_init(|| AnchorIds::from_text(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/interactions.ron"))))
    }
}

/// What a person is using, as the network carries it: an anchor (an id of
/// data/characters/interactions.ron) and how far into it they are (0 entering, 1 holding, 2 leaving:
/// `humanity_animation::interact::Phase`'s codes, which never change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    pub anchor: String,
    pub phase: u8,
}

/// The highest phase code.
pub const ACT_PHASE_MAX: u8 = 2;

impl Act {
    /// The `act` field: `["chair_seat", 1]`.
    pub fn to_json(&self) -> Value {
        Value::from(vec![Value::from(self.anchor.clone()), Value::from(self.phase)])
    }

    /// The act a message's `act` field says, clamped by the one reader: None for an anchor the
    /// shipped game does not know (or one too long), a phase that is not 0 to 2, or anything that
    /// is not `[string, whole number]`. A missing field (using nothing, or an older app) is None.
    pub fn from_json(v: Option<&Value>, ids: &AnchorIds) -> Option<Act> {
        let a = v?.as_array()?;
        if a.len() != 2 {
            return None;
        }
        let anchor = a[0].as_str()?;
        if anchor.is_empty() || anchor.len() > ANCHOR_ID_MAX || !ids.ids.contains(anchor) {
            return None;
        }
        let phase = u8::try_from(a[1].as_u64()?).ok().filter(|p| *p <= ACT_PHASE_MAX)?;
        Some(Act { anchor: anchor.to_string(), phase })
    }
}

/// Where a person looks and what they are using, as one movement message carries them: the look's
/// pitch (radians, up positive) and their act. Both read through their one reader each
/// (`from_json`), written left out when there is nothing to say (`write_json`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Doing {
    pub pitch: f32,
    pub act: Option<Act>,
}

impl Doing {
    /// The pitch and act a message says, each clamped (`pitch_from_json`, `Act::from_json`).
    pub fn from_json(msg: &Value, ids: &AnchorIds) -> Self {
        Doing { pitch: pitch_from_json(msg.get("pitch")), act: Act::from_json(msg.get("act"), ids) }
    }

    /// Put the `pitch` and `act` fields in a message, each only when it says something.
    pub fn write_json(&self, msg: &mut Value) {
        if let (Some(p), Some(obj)) = (pitch_to_json(self.pitch), msg.as_object_mut()) {
            obj.insert("pitch".into(), p);
        }
        if let (Some(a), Some(obj)) = (&self.act, msg.as_object_mut()) {
            obj.insert("act".into(), a.to_json());
        }
    }
}

#[cfg(test)]
mod act_tests {
    use super::*;
    use serde_json::json;

    /// THE PITCH AND THE ACT ON THE WIRE, CLAMPED (rung 2b): a pitch round-trips to the degree and
    /// is left out when level; past +-89 degrees it is clamped; anything not a whole number reads as
    /// level. An act round-trips; an unknown anchor, a phase out of range, a wrong shape and a
    /// missing field read as using nothing. The shipped ids are the shipped file's.
    /// Seen red 2026-11-10 with the reader taking any short enough anchor id ("an unknown anchor is
    /// dropped", left: Some(Act { anchor: "throne", phase: 1 })) and with the pitch unclamped ("past
    /// straight up: clamped", left: 120, right: 89).
    #[test]
    fn the_pitch_and_the_act_round_trip_and_anything_else_is_dropped() {
        let up = 30f32.to_radians();
        assert!((pitch_from_json(pitch_to_json(up).as_ref()) - up).abs() < 1e-4);
        assert_eq!(pitch_to_json(0.2f32.to_radians()), None, "level is left out");
        assert_eq!(pitch_to_json(f32::NAN), None);
        assert_eq!(pitch_from_json(Some(&json!(120))).to_degrees().round() as i64, 89, "past straight up: clamped");
        assert_eq!(pitch_from_json(Some(&json!(-200))).to_degrees().round() as i64, -89);
        for bad in [json!(12.5), json!("12"), json!(null), json!([1])] {
            assert_eq!(pitch_from_json(Some(&bad)), 0.0, "{bad} reads as level");
        }
        assert_eq!(pitch_from_json(None), 0.0, "an older message reads as level");

        let ids = AnchorIds::from_text("[(id: \"chair_seat\", kind: Sit, root: (0.0, 0.0, 0.3), contacts: [(part: Hips, at: (0.0, 0.47, 0.0))]), (id: \"ladder_climb\", kind: Climb)]");
        assert_eq!(ids.ids.len(), 2, "the ids are read past each row's other fields: {ids:?}");
        let act = Act { anchor: "chair_seat".into(), phase: 1 };
        assert_eq!(Act::from_json(Some(&act.to_json()), &ids), Some(act));
        assert_eq!(Act::from_json(Some(&json!(["throne", 1])), &ids), None, "an unknown anchor is dropped");
        assert_eq!(Act::from_json(Some(&json!(["chair_seat", 3])), &ids), None, "a phase out of range is dropped");
        assert_eq!(Act::from_json(Some(&json!(["chair_seat", -1])), &ids), None);
        assert_eq!(Act::from_json(Some(&json!(["chair_seat", 1.5])), &ids), None);
        assert_eq!(Act::from_json(Some(&json!(["chair_seat"])), &ids), None);
        assert_eq!(Act::from_json(Some(&json!({"anchor": "chair_seat"})), &ids), None);
        assert_eq!(Act::from_json(Some(&json!([1, 1])), &ids), None);
        assert_eq!(Act::from_json(None, &ids), None, "a message without it uses nothing");
        let long = "a".repeat(ANCHOR_ID_MAX + 1);
        let with_long = AnchorIds { ids: [long.clone()].into_iter().collect() };
        assert_eq!(Act::from_json(Some(&json!([long, 0])), &with_long), None, "too long an id is dropped");
        // The shipped file's ids are read (the rows parse past their other fields).
        let shipped = AnchorIds::shipped();
        assert!(shipped.ids.contains("chair_seat") && shipped.ids.contains("ladder_climb"), "the shipped anchors: {:?}", shipped.ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// THE STANCE ON THE WIRE: every stance round-trips with and without the off-the-ground flag;
    /// standing on the ground is left out of the message; an older message without the field reads
    /// as standing; and whatever else arrives (an unknown code, a negative, a fraction, a string, a
    /// number past a byte) is clamped to standing, never trusted.
    /// Seen red 2026-11-09 with `from_wire` reading the code before masking the flag ("crouching in
    /// the air round-trips": Stand, airborne, for Crouch, airborne).
    #[test]
    fn the_stance_round_trips_and_anything_else_reads_as_standing() {
        for code in 0..7u8 {
            for airborne in [false, true] {
                let p = Posture { stance: Stance::from_code(code).unwrap(), airborne };
                assert_eq!(Posture::from_wire(p.to_wire() as u64), p, "crouching in the air round-trips");
                assert_eq!(Posture::from_json(p.to_json().as_ref()), p);
            }
        }
        assert_eq!(Posture::default().to_json(), None, "standing is left out");
        let old = json!({ "type": "game_position_update", "position": [1.0, 2.0, 3.0] });
        assert_eq!(Posture::from_json(old.get("stance")), Posture::default(), "an older message reads as standing");
        for bad in [json!(7), json!(127), json!(-1), json!(1.5), json!("1"), json!(256), json!(u64::MAX), json!(null), json!([1])] {
            let p = Posture::from_json(Some(&bad));
            assert_eq!(p.stance, Stance::Stand, "{bad} reads as standing");
        }
        assert!(!Posture::from_json(Some(&json!(256))).airborne, "past a byte is not off the ground");
        assert_eq!(Posture::from_json(Some(&json!(129))), Posture { stance: Stance::Crouch, airborne: true });
    }
}
