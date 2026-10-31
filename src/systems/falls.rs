//! Falls: how hard a body lands, and what that does to it (2026-10-20).
//!
//! Until 2026-10-20 a player could drop any distance and take nothing: both of the game's
//! walking models stopped a fall at the floor and threw the speed away. Now the speed the
//! ground stops is an injury (`wounds::Injury::Fall`), turned by the food system's body pass
//! into health lost and, past a threshold, a wound that slows walking while it heals.
//!
//! IMPACT SPEED, NOT HEIGHT. What hurts a body is how fast it is going when the ground stops it,
//! and that is the same rule under a ship's spin gravity, on the Moon and on Earth: a 4 m drop
//! on Earth lands at 8.9 m/s, the same drop on the Moon at 3.6 m/s, and it takes a 24 m drop on
//! the Moon to land as hard as the 4 m one on Earth. So the thresholds in data/medical/falls.ron
//! are speeds, each derived from the fall height a published source gives with v = sqrt(2 g h).
//!
//! WHERE A LANDING IS SEEN. The game walks a body two ways, and each now reports the speed its
//! floor stopped:
//! - Aboard (the ship and the homestead, the Normal-mode game): `CameraController::
//!   update_first_person` in src/renderer/camera.rs, the floor-and-gravity walk.
//! - On a planet's surface: `surface_move::radial_step`, the radial ballistics, which reports
//!   `RadialStep::impact_m_s`; lib.rs hands it to `CameraController::surface_step`.
//!
//! Both feed one `FallWatch` (`physics::landing`) on the controller, which decides whether a landing is a fall that
//! can hurt: never one that began in dev flight (fly mode, the F9 hover, the planet's flight
//! band) or after something else moved the body (a teleport, driving, the rig's camera hold),
//! and never one onto water. lib.rs takes the landing once a frame (`CameraController::
//! take_landing`) and gives it to the player's body (`player_landed`).
//!
//! TWO MODES (the house rule for deep systems), the Settings > Gameplay > Illness and injury
//! setting that wounds already follow (`illness::Mode`): Realistic uses the speeds as cited;
//! Forgiving, the default, reads every fall at `forgiving.speed_share` of its speed, so only
//! bigger falls hurt, and they hurt less.
//!
//! Pure data and rules with no engine state, so it compiles in every feature set.

use std::path::Path;

use serde::Deserialize;

use crate::systems::illness::Mode;

/// Forgiving's reading of a fall (falls.ron `forgiving`).
#[derive(Debug, Clone, Deserialize)]
pub struct ForgivingFalls {
    /// The share of its impact speed a fall is read at in Forgiving (0..1).
    pub speed_share: f32,
}

/// One point of the harm curve: landing at `m_s` takes `health` of the body's 100.
#[derive(Debug, Clone, Deserialize)]
pub struct HarmPoint {
    pub m_s: f32,
    pub health: f32,
}

/// One step of injury: from `from_m_s` a landing leaves the wound `effect` (a wounds.ron row).
#[derive(Debug, Clone, Deserialize)]
pub struct FallStep {
    pub from_m_s: f32,
    pub effect: String,
}

/// data/medical/falls.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Falls {
    /// The death screen's words when a fall is what killed the player.
    pub cause: String,
    /// Said first when a landing hurts ("You land hard.").
    pub landed: String,
    /// Forgiving's reading of a fall.
    pub forgiving: ForgivingFalls,
    /// Health a landing takes by its speed, slowest first: none below the first point, straight
    /// lines between points, and past the last one the last line carries on.
    #[serde(default)]
    pub harm: Vec<HarmPoint>,
    /// The wound a landing leaves, mildest first: the last step its speed reaches.
    #[serde(default)]
    pub steps: Vec<FallStep>,
}

impl Default for Falls {
    /// No fall data: no fall hurts.
    fn default() -> Self {
        Self { cause: String::new(), landed: String::new(), forgiving: ForgivingFalls { speed_share: 1.0 }, harm: Vec::new(), steps: Vec::new() }
    }
}

impl Falls {
    /// Path of the file, relative to the data directory.
    pub const FILE: &'static str = "medical/falls.ron";

    /// Disk first (modding), the copy built into the game when the disk copy is missing or this
    /// version cannot read it (BUG-163). With neither, no fall hurts: loud in the log, never a
    /// crash.
    pub fn load(data_dir: &Path) -> Self {
        crate::embedded_data::load_text_or_embedded(data_dir, Self::FILE, Self::from_ron).unwrap_or_else(|e| {
            log::warn!("{e}; no fall hurts until it is fixed");
            Self::default()
        })
    }

    /// Parse the file's text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The speed a landing at `impact_m_s` is read at in this mode.
    pub fn read_speed(&self, impact_m_s: f32, mode: Mode) -> f32 {
        let v = impact_m_s.max(0.0);
        match mode {
            Mode::Realistic => v,
            Mode::Forgiving => v * self.forgiving.speed_share.clamp(0.0, 1.0),
        }
    }

    /// Health a landing read at `v` m/s takes (`harm`).
    pub fn harm_at(&self, v: f32) -> f64 {
        let pts = &self.harm;
        let Some(first) = pts.first() else { return 0.0 };
        if v < first.m_s {
            return 0.0;
        }
        // The segment `v` falls on, or the last one carried on past its end.
        let i = pts.windows(2).position(|w| v < w[1].m_s).unwrap_or(pts.len().saturating_sub(2));
        let (a, b) = match (pts.get(i), pts.get(i + 1)) {
            (Some(a), Some(b)) if b.m_s > a.m_s => (a, b),
            _ => return f64::from(first.health.max(0.0)),
        };
        let t = f64::from((v - a.m_s) / (b.m_s - a.m_s));
        (f64::from(a.health) + t * f64::from(b.health - a.health)).max(0.0)
    }

    /// The wound a landing read at `v` m/s leaves: the last step it reaches.
    pub fn wound_at(&self, v: f32) -> Option<&str> {
        self.steps.iter().filter(|s| v >= s.from_m_s).last().map(|s| s.effect.as_str())
    }

    /// The landing speed (as read, m/s) that takes `health`: the harm curve run backwards, its
    /// last line carried on past its end. None when the curve never reaches it.
    pub fn speed_taking(&self, health: f32) -> Option<f32> {
        let pts = &self.harm;
        for w in pts.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if health <= b.health && health >= a.health && b.health > a.health {
                return Some(a.m_s + (health - a.health) / (b.health - a.health) * (b.m_s - a.m_s));
            }
        }
        let n = pts.len();
        if n >= 2 {
            let (a, b) = (&pts[n - 2], &pts[n - 1]);
            if health > b.health && b.health > a.health && b.m_s > a.m_s {
                return Some(b.m_s + (health - b.health) / (b.health - a.health) * (b.m_s - a.m_s));
            }
        }
        None
    }

    /// The slowest landing that does anything in Realistic: the first harm point or step.
    pub fn first_harm_m_s(&self) -> Option<f32> {
        let a = self.harm.iter().find(|p| p.health > 0.0).map(|p| p.m_s);
        let b = self.steps.first().map(|s| s.from_m_s);
        match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

// The landing's physics (how fast the floor stopped a body, `drop_height_m`, `landing_speed`)
// and the watch that decides whether it was a fall that can hurt (`FallWatch`, `LANDING_M_S`)
// are `physics::landing`'s, in the core, since the crate split's rung 0: the renderer's camera
// controller keeps the watch and measures the aboard landing, and must not reach up into the
// simulation for it. What a landing does to the body stays here.

/// The player landed at `impact_m_s`: give it to their body (the one Controllable with the body's
/// meters), whose next body pass makes it the fall's injury. A dead body takes nothing (the
/// food system drops what waits on it).
pub fn player_landed(world: &mut hecs::World, impact_m_s: f32) {
    use crate::ecs::components::{Controllable, Dead, StatusEffects, Vitals};
    let body = world
        .query::<(&Controllable, &Vitals, &StatusEffects, Option<&Dead>)>()
        .iter()
        .find(|(_, (_, _, _, dead))| dead.is_none())
        .map(|(e, _)| e);
    // One line a landing (only landings that can hurt reach here, so a jump's is the most there is):
    // what a rig reads to see a fall reached the body, and what anyone reading run.log after
    // "my health dropped for no reason" needs (scripts/verify-falls.js).
    match body {
        Some(body) => {
            log::info!("[Falls] the player landed at {impact_m_s:.2} m/s");
            crate::systems::wounds::injure(world, body, crate::systems::wounds::Injury::Fall { impact_m_s });
        }
        None => log::info!("[Falls] a landing at {impact_m_s:.2} m/s found no living body to hurt"),
    }
}
