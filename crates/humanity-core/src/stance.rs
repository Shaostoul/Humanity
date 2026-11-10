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
//! Only standing and crouching are sent today (the rung that builds sitting, climbing, swimming,
//! flight and zero-g animation sends theirs); the codes are fixed now so an app that does not know a
//! newer stance still reads the number and draws the person standing.

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
