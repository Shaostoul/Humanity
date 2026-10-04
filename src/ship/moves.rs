//! Moving aboard the shared ship (increment 4 of docs/design/ship-homes-and-logistics.md,
//! "getting around at ship scale", and its section 5.10): how fast the relay lets a person go,
//! how a game says a fast move was a real one, and how far away another person still counts as
//! in view. Shared by the relay (src/relay/handlers/move_check.rs, game_interest.rs) and the game
//! (src/engine/move_check.rs), so the two read one set of rules: data/ship/shared_world.ron.
//!
//! Until this increment the relay refused any update more than 100 m from where it held a
//! player and kept them where they were, with no time in the rule at all: a modified game could
//! claim about 1.5 km/s by staying under 100 m per update at 15 updates a second, while an
//! honest one that jumped (shutting the build editor from the far end of First Street, a
//! teleporter) was left frozen on everyone else's screen, every later update refused too.

use glam::Vec3;
use serde::Deserialize;

/// How far the game may stand from where the relay holds the player before a welcome stands
/// them back there, metres, and the farthest a reconnect or shutting the build editor may move
/// them in one update. The relay used to refuse any update more than 100 m from where it held
/// them; 90 left a margin for one update's walk, and the welcome's rule and the editor's are
/// kept at it so a returning or building player is never moved farther than they could be
/// before (engine/home_plot.rs `stand_where_held`, `editor_close_spot`). One place for both
/// sides since increment 4: the relay's speed check grants a reconnect at most this.
pub const FAR_FROM_HELD_M: f32 = 90.0;

/// The rules for moving aboard and for who hears about it (data/ship/shared_world.ron, where
/// each number is explained).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SharedWorldRules {
    pub moving: MovingRules,
    pub delivery: DeliveryRules,
}

/// How fast a person may go (data/ship/shared_world.ron `moving`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MovingRules {
    pub on_foot_mps: f32,
    pub vertical_mps: f32,
    pub jitter_margin: f32,
    pub banked_s: f32,
    pub slack_m: f32,
    pub correction_gap_s: f32,
    pub correction_resend_s: f32,
}

/// Who hears about a move (data/ship/shared_world.ron `delivery`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeliveryRules {
    pub in_view_m: f32,
    pub out_of_view_m: f32,
}

impl SharedWorldRules {
    /// Read the rules from `data_dir`/ship/shared_world.ron, else the copy built into the exe
    /// (named in the log, after the disk read: scripts/lib/compiled-in.js counts the file as
    /// disk-first by that). A file that does not parse or fails `validate` is never half-used.
    pub fn load(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("ship").join("shared_world.ron");
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(&path);
        let from_disk = match &text {
            Ok(t) => Self::parse(t).map_err(|e| format!("{shown} does not load ({e})")),
            Err(e) => Err(format!("{shown} could not be read ({e})")),
        };
        match from_disk {
            Ok(rules) => rules,
            Err(why) => {
                crate::embedded_data::note_builtin_copy("ship/shared_world.ron", why);
                Self::parse(include_str!("../../data/ship/shared_world.ron"))
                    .expect("the built-in data/ship/shared_world.ron parses (moves.rs tests)")
            }
        }
    }

    /// Parse and check the rules (`validate`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let rules: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        rules.validate()?;
        Ok(rules)
    }

    /// Every number finite and of a size that means something: speeds and radii above zero,
    /// the margins and allowances not negative, and the out-of-view radius at least the in-view
    /// one (RON reads `NaN` and `inf`, and a negative speed would correct every move).
    pub fn validate(&self) -> Result<(), String> {
        let m = &self.moving;
        let d = &self.delivery;
        let positive = [
            ("on_foot_mps", m.on_foot_mps),
            ("vertical_mps", m.vertical_mps),
            ("banked_s", m.banked_s),
            ("in_view_m", d.in_view_m),
            ("out_of_view_m", d.out_of_view_m),
        ];
        for (name, v) in positive {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("{name} must be a number above 0, not {v}"));
            }
        }
        let not_negative = [
            ("jitter_margin", m.jitter_margin),
            ("slack_m", m.slack_m),
            ("correction_gap_s", m.correction_gap_s),
            ("correction_resend_s", m.correction_resend_s),
        ];
        for (name, v) in not_negative {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("{name} must be a number of 0 or more, not {v}"));
            }
        }
        if d.out_of_view_m < d.in_view_m {
            return Err(format!("out_of_view_m ({}) must be at least in_view_m ({})", d.out_of_view_m, d.in_view_m));
        }
        Ok(())
    }
}

/// How a move that is not walking happened, as a game declares it in its position update
/// (`"moved"`), so the relay can tell a real fast move from one nobody could make. The relay
/// checks each against what it knows (move_check.rs): it never takes the game's word alone.
#[derive(Debug, Clone, PartialEq)]
pub enum MoveDecl {
    /// Through a transit link (a teleporter, src/ship/transit.rs): stepped into structure `from`
    /// of zone `zone` at `from_at` and came out at structure `to`, `to_at` (ship metres, floor
    /// height). A link in a shared zone is checked against the relay's own ship file by its ids;
    /// one in the player's own home (zone "home", which the relay has no copy of) must have both
    /// ends on the player's own plot.
    Link { zone: String, from: String, to: String, from_at: Vec3, to_at: Vec3 },
    /// Driving a vehicle (its item id, data/vehicles/kits.ron): its own speed applies.
    Vehicle { vehicle: String },
    /// Shutting the build editor stood the player at their build spot, which is on their own
    /// plot (engine/home_plot.rs `editor_close_spot`).
    Editor,
}

impl MoveDecl {
    /// The wire form, the value of a position update's `"moved"`.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            MoveDecl::Link { zone, from, to, from_at, to_at } => serde_json::json!({
                "by": "link",
                "zone": zone,
                "from": from,
                "to": to,
                "from_at": [from_at.x, from_at.y, from_at.z],
                "to_at": [to_at.x, to_at.y, to_at.z],
            }),
            MoveDecl::Vehicle { vehicle } => serde_json::json!({ "by": "vehicle", "vehicle": vehicle }),
            MoveDecl::Editor => serde_json::json!({ "by": "editor" }),
        }
    }

    /// Read a position update's `"moved"`. None for anything that is not one of the forms above
    /// (the move is then judged as walking).
    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let p = |k: &str| -> Option<Vec3> {
            let a = v.get(k)?.as_array()?;
            if a.len() != 3 {
                return None;
            }
            let p = Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32);
            p.is_finite().then_some(p)
        };
        match v.get("by").and_then(|b| b.as_str())? {
            "link" => Some(MoveDecl::Link { zone: s("zone")?, from: s("from")?, to: s("to")?, from_at: p("from_at")?, to_at: p("to_at")? }),
            "vehicle" => Some(MoveDecl::Vehicle { vehicle: s("vehicle")? }),
            "editor" => Some(MoveDecl::Editor),
            _ => None,
        }
    }
}

/// Distance across the floor (x and z), metres.
pub fn floor_distance(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> SharedWorldRules {
        SharedWorldRules::parse(include_str!("../../data/ship/shared_world.ron")).expect("the shipped rules parse")
    }

    /// The shipped file parses, validates, and the disk read finds it (the relay and the game
    /// load it with `load("data")` from the repo root in tests).
    #[test]
    fn the_shipped_rules_load() {
        let r = shipped();
        assert!(r.delivery.out_of_view_m > r.delivery.in_view_m);
        assert_eq!(SharedWorldRules::load(std::path::Path::new("data")), r, "the disk copy is the shipped one");
    }

    /// Nonsense is refused, never half-used: a negative speed, NaN, and radii the wrong way
    /// round. Seen red 2026-10-04 with `validate` returning Ok(()) at once: "a NaN speed loaded".
    #[test]
    fn rules_that_mean_nothing_are_refused() {
        let text = include_str!("../../data/ship/shared_world.ron");
        assert!(SharedWorldRules::parse(&text.replace("on_foot_mps: 25.0", "on_foot_mps: NaN")).is_err(), "a NaN speed loaded");
        assert!(SharedWorldRules::parse(&text.replace("on_foot_mps: 25.0", "on_foot_mps: -1.0")).is_err());
        assert!(SharedWorldRules::parse(&text.replace("slack_m: 1.0", "slack_m: inf")).is_err());
        assert!(SharedWorldRules::parse(&text.replace("out_of_view_m: 300.0", "out_of_view_m: 100.0")).is_err());
    }

    /// The declarations survive the wire both ways, and anything else reads as walking. Seen red
    /// 2026-10-04 with "editor" not read: "assertion `left == right` failed (left: None, right:
    /// Some(Editor))".
    #[test]
    fn a_declared_move_survives_the_wire() {
        let all = [
            MoveDecl::Link {
                zone: "home".into(),
                from: "teleporter-west".into(),
                to: "teleporter-east".into(),
                from_at: Vec3::new(22.5, 0.0, 119.0),
                to_at: Vec3::new(31.0, 0.0, 179.0),
            },
            MoveDecl::Vehicle { vehicle: "rover_0".into() },
            MoveDecl::Editor,
        ];
        for d in all {
            assert_eq!(MoveDecl::from_json(&d.to_json()).as_ref(), Some(&d));
        }
        assert_eq!(MoveDecl::from_json(&serde_json::json!({"by": "warp"})), None);
        assert_eq!(MoveDecl::from_json(&serde_json::json!({"by": "link", "zone": "home"})), None, "a link without its ends");
        assert_eq!(MoveDecl::from_json(&serde_json::json!(null)), None);
    }

    /// THE ON-FOOT LIMIT COVERS THE FASTEST LEGITIMATE WALK: the walk and the sprint the
    /// movement code uses, times every speed buff of data/status_effects.csv at its most stacks,
    /// times every speed modifier of data/equipment.csv above 1. A new buff or a faster walk that
    /// passes the limit would have the relay correct honest players, so it fails here first.
    /// Seen red 2026-10-04 with the limit at 9.5 (the plain sprint): "the fastest legitimate walk
    /// is 24.11 m/s, over the on-foot limit of 9.50".
    #[cfg(feature = "native")]
    #[test]
    fn the_on_foot_limit_covers_the_fastest_legitimate_walk() {
        use crate::renderer::camera::{SPRINT_FACTOR, WALK_SPEED_MPS};
        let mut fastest = WALK_SPEED_MPS * SPRINT_FACTOR;
        // status_effects.csv: id,name,type,duration_s,stackable,max_stacks,tick_interval_s,stat_modifier,...
        let effects = std::fs::read_to_string("data/status_effects.csv").expect("data/status_effects.csv");
        for line in effects.lines().filter(|l| !l.starts_with('#') && !l.starts_with("id,") && !l.trim().is_empty()) {
            let cols: Vec<&str> = line.split(',').collect();
            let stacks: i32 = cols.get(5).and_then(|s| s.trim().parse().ok()).unwrap_or(1).max(1);
            if let Some(m) = cols.get(7).and_then(|m| m.strip_prefix("speed:")).and_then(|m| m.strip_suffix(":multiply")) {
                let v: f32 = m.parse().expect("a speed modifier is a number");
                if v > 1.0 {
                    fastest *= v.powi(stacks);
                }
            }
        }
        let equipment = std::fs::read_to_string("data/equipment.csv").expect("data/equipment.csv");
        for line in equipment.lines().filter(|l| !l.starts_with('#')) {
            for field in line.split(',') {
                for m in field.split('|') {
                    if let Some(v) = m.strip_prefix("speed:").and_then(|m| m.strip_suffix(":multiply")) {
                        let v: f32 = v.parse().expect("a speed modifier is a number");
                        if v > 1.0 {
                            fastest *= v;
                        }
                    }
                }
            }
        }
        let limit = shipped().moving.on_foot_mps;
        assert!(fastest > WALK_SPEED_MPS * SPRINT_FACTOR, "the buffs were found: {fastest}");
        assert!(fastest <= limit, "the fastest legitimate walk is {fastest:.2} m/s, over the on-foot limit of {limit:.2}");
    }
}
