//! CARRYING WEIGHT (BUG-136, 2026-10-04): how much the player can carry where
//! they stand, and what carrying more than that does to them.
//!
//! Until this file the Inventory page showed a fixed 50 kg limit and being
//! over it did nothing at all: the inventory system set an `encumbered` flag
//! that nothing read. The operator's decision, 2026-10-04, verbatim: "slower
//! walking in realistic mode and no jumping in nonzero G based on
//! weight/mass. We can obviously carry heavier in low-g to zero-g but, mass
//! still applies."
//!
//! THE RULES, each a pure function below with its own test:
//!
//! 1. THE LIMIT FOLLOWS GRAVITY. The inventory's capacity (50 kg) plus what
//!    worn gear adds (`carry_capacity` in data/equipment.csv: a large backpack
//!    adds 25 kg) is what a body carries comfortably at 1 g, 9.81 m/s^2, the
//!    homestead's gravity and Earth's. What the legs hold up is the load's
//!    WEIGHT, its mass times the local gravity, so the same legs carry 9.81 / g
//!    times the mass: 2.6 times as much on Mars, 6 times on the Moon. Below
//!    `WEIGHTLESS_BELOW_M_S2` nothing weighs anything and there is no limit.
//! 2. OVERLOADED, REALISTIC MODE: walking slows by how far over the limit the
//!    load is (10% over walks at 90% speed, half again over at half speed,
//!    never below a crawl), and there is no jumping while gravity holds you
//!    down (an overload needs weight, so it only ever happens under gravity).
//! 3. MASS STILL APPLIES, REALISTIC MODE: the legs push off with the same
//!    effort whatever is carried, and the load is extra mass to launch, so a
//!    jump leaves the ground slower. Kinetic energy is half m v squared, so for
//!    the same push the launch speed goes as the square root of body mass over
//!    body plus load. This holds in low gravity too: on the Moon a 150 kg load
//!    is well under the limit and still cuts the launch speed to 56%.
//! 4. FORGIVING MODE (the default, the house rule for deep systems): the same
//!    limit and the same readout, and a clear warning when over it. Nothing
//!    about movement changes.
//!
//! WHAT THE MOVEMENT MODEL DOES WITH THIS (engine::carry_load applies it each
//! frame): the walking speed factor multiplies the controller's speed
//! multiplier, which both the homestead walk and the planet surface walk
//! read; the jump scale multiplies the homestead jump's launch speed and the
//! planet surface's Space thrust. Starting and stopping are instant in both
//! walking models today (no horizontal inertia), and there is no zero-g
//! pushing model (the homestead always has its game.csv gravity, dev flight
//! is a noclip camera), so the jump is the one place the movement model has
//! momentum for the carried mass to act on.
//!
//! Pure std, no ECS, no GPU, so it compiles in every feature set and the tests
//! run standalone: `rustc --test --edition 2021 src/systems/encumbrance.rs`.

/// One g, m/s^2: the homestead's interior gravity (data/game.csv
/// `gravity_m_s2`) and Earth's surface gravity in data/planets/earth.ron. The
/// inventory's capacity is a limit AT this gravity.
pub const ONE_G_M_S2: f32 = 9.81;

/// Below this gravity nothing weighs enough to overload anyone: about 1% of
/// a g. The homestead's gravity knob and the planet surface sampler both floor
/// at 0.01 m/s^2 rather than a literal zero, so a declared zero-g band lands
/// here.
pub const WEIGHTLESS_BELOW_M_S2: f32 = 0.1;

/// The slowest an overloaded walker goes, as a share of normal speed: a crawl,
/// never a dead stop, so a player can always shuffle to somewhere to drop the
/// load.
pub const CRAWL_SPEED: f32 = 0.15;

/// The body that pushes off, kg: the standard 70 kg person the body heat model
/// uses (`body_heat::BODY_MASS_KG`, the Gagge model's standard person).
pub const BODY_MASS_KG: f32 = 70.0;

/// DataStore key the engine publishes the player's local gravity under each
/// frame (m/s^2, f32), so the inventory system's `encumbered` flag uses the
/// same limit as the page and the movement. Absent (headless tests, the
/// relay) means 1 g.
pub const LOCAL_G_KEY: &str = "player_local_gravity";

/// Where Settings keeps the switch, for the sentences that point at it.
pub const SETTING_PATH: &str = "Settings > Gameplay > Carrying weight";

/// The comfortable carry limit where the player stands, kg, for a limit of
/// `limit_1g_kg` at one g. None when weightless: there is no limit.
pub fn limit_here_kg(limit_1g_kg: f32, g_m_s2: f32) -> Option<f32> {
    if !(g_m_s2 >= WEIGHTLESS_BELOW_M_S2) {
        // Also catches NaN: an unknown gravity is not a reason to slow anyone.
        return None;
    }
    Some(limit_1g_kg.max(0.0) * ONE_G_M_S2 / g_m_s2)
}

/// How heavy the load is against the limit: 1.0 is exactly at it. Zero when
/// weightless (no limit) or when nothing is carried.
pub fn overload_ratio(carried_kg: f32, limit_here: Option<f32>) -> f32 {
    match limit_here {
        Some(limit) if limit > 0.0 => carried_kg.max(0.0) / limit,
        // A zero limit with anything in hand is as overloaded as it gets.
        Some(_) if carried_kg > 0.0 => f32::INFINITY,
        _ => 0.0,
    }
}

/// Walking speed, as a share of normal, for a load at `ratio` of the limit:
/// full speed up to the limit, then each tenth over takes a tenth off, never
/// below `CRAWL_SPEED`.
pub fn walk_speed_factor(ratio: f32) -> f32 {
    if !(ratio > 1.0) {
        return 1.0;
    }
    (2.0 - ratio).max(CRAWL_SPEED)
}

/// The jump's launch speed, as a share of an unloaded one, for `carried_kg`
/// on a `BODY_MASS_KG` body: the same push, more mass to launch.
pub fn mass_jump_scale(carried_kg: f32) -> f32 {
    (BODY_MASS_KG / (BODY_MASS_KG + carried_kg.max(0.0))).sqrt()
}

/// Which of the two modes the Settings switch picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CarryMode {
    /// The default: a warning only.
    #[default]
    Forgiving,
    /// The load slows you, an overload stops jumps, and mass weighs on jumps.
    Realistic,
}

impl CarryMode {
    /// From the saved setting (`AppConfig::carry_realistic`).
    pub fn from_realistic(realistic: bool) -> Self {
        if realistic {
            CarryMode::Realistic
        } else {
            CarryMode::Forgiving
        }
    }
}

/// What the player carries, as the inventory system measured it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CarryInput {
    /// Mass of everything carried, kg (`Inventory::weight_current`).
    pub carried_kg: f32,
    /// The inventory's own limit at 1 g, kg (`Inventory::weight_capacity`).
    pub capacity_kg: f32,
    /// What worn gear adds at 1 g, kg (`Inventory::carry_bonus_kg`).
    pub bonus_kg: f32,
    /// Bulk carried and the room for it, litres. A readout only: no movement
    /// rule reads volume.
    pub volume_l: f32,
    pub volume_capacity_l: f32,
}

/// The load, the limit where the player stands, and what it does to them.
/// Published to the GUI each frame as `GuiState::carry`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarryState {
    pub input: CarryInput,
    /// The gravity the limit was worked out for, m/s^2.
    pub g_m_s2: f32,
    pub mode: CarryMode,
    /// The limit here, kg; None when weightless.
    pub limit_here_kg: Option<f32>,
    /// Load over the limit here (whatever the mode).
    pub overloaded: bool,
    /// Walking speed as a share of normal (1.0 unless Realistic and over).
    pub speed_factor: f32,
    /// Jump launch speed as a share of an unloaded one (1.0 in Forgiving,
    /// 0.0 when Realistic and over).
    pub jump_scale: f32,
}

impl Default for CarryState {
    /// Nothing carried at 1 g with the inventory's default 50 kg limit: what
    /// the page shows before a player inventory exists.
    fn default() -> Self {
        evaluate(
            CarryInput { capacity_kg: 50.0, volume_capacity_l: 65.0, ..CarryInput::default() },
            ONE_G_M_S2,
            CarryMode::Forgiving,
        )
    }
}

/// THE ONE PLACE the rules above are combined.
pub fn evaluate(input: CarryInput, g_m_s2: f32, mode: CarryMode) -> CarryState {
    let limit_1g = input.capacity_kg.max(0.0) + input.bonus_kg.max(0.0);
    let limit_here = limit_here_kg(limit_1g, g_m_s2);
    let ratio = overload_ratio(input.carried_kg, limit_here);
    let overloaded = ratio > 1.0;
    let (speed_factor, jump_scale) = match mode {
        CarryMode::Forgiving => (1.0, 1.0),
        CarryMode::Realistic => {
            // An overload only exists under gravity (`limit_here_kg`), and
            // under gravity an overloaded body does not leave the ground.
            let jump = if overloaded { 0.0 } else { mass_jump_scale(input.carried_kg) };
            (walk_speed_factor(ratio), jump)
        }
    };
    CarryState { input, g_m_s2, mode, limit_here_kg: limit_here, overloaded, speed_factor, jump_scale }
}

/// A share as a whole percentage, "80%".
fn pct(share: f32) -> String {
    format!("{:.0}%", (share * 100.0).round())
}

impl CarryState {
    /// The limit at 1 g, kg: the inventory's own plus what is worn.
    pub fn limit_1g_kg(&self) -> f32 {
        self.input.capacity_kg.max(0.0) + self.input.bonus_kg.max(0.0)
    }

    /// The load against the limit here, 0 to 1, for the tile's bar.
    pub fn fraction(&self) -> f32 {
        overload_ratio(self.input.carried_kg, self.limit_here_kg).clamp(0.0, 1.0)
    }

    /// The Weight tile's value: "62.0 / 50.0 kg", or the mass alone when
    /// weightless.
    pub fn tile_value(&self) -> String {
        match self.limit_here_kg {
            Some(limit) => format!("{:.1} / {:.1} kg", self.input.carried_kg, limit),
            None => format!("{:.1} kg, weightless", self.input.carried_kg),
        }
    }

    /// The sentences under the Status tiles: where the limit comes from and,
    /// when it matters, why the player is slow or cannot jump.
    pub fn tile_note(&self) -> String {
        let limit_1g = self.limit_1g_kg();
        let mut s = if self.input.bonus_kg > 0.0 {
            format!(
                "You can carry {:.0} kg comfortably at 1 g: {:.0} kg, plus {:.0} kg from what you wear.",
                limit_1g, self.input.capacity_kg, self.input.bonus_kg
            )
        } else {
            format!("You can carry {:.0} kg comfortably at 1 g. A backpack raises it.", limit_1g)
        };
        match self.limit_here_kg {
            None => s.push_str(
                " Here you are weightless, so the load does not hold you down, but its mass is still there.",
            ),
            Some(limit) if (self.g_m_s2 / ONE_G_M_S2 - 1.0).abs() > 0.02 => s.push_str(&format!(
                " Gravity here is {:.2} g, so the limit is {:.0} kg.",
                self.g_m_s2 / ONE_G_M_S2,
                limit
            )),
            Some(_) => {}
        }
        if self.overloaded {
            let over = self.input.carried_kg - self.limit_here_kg.unwrap_or(0.0);
            match self.mode {
                CarryMode::Realistic => s.push_str(&format!(
                    " Overloaded by {over:.1} kg: you walk at {} speed and cannot jump. Drop or store something, or wear a pack.",
                    pct(self.speed_factor)
                )),
                CarryMode::Forgiving => s.push_str(&format!(
                    " Overloaded by {over:.1} kg. Forgiving mode does not slow you; Realistic mode ({SETTING_PATH}) would."
                )),
            }
        } else if self.mode == CarryMode::Realistic && self.jump_scale < 0.995 {
            s.push_str(&format!(
                " The load is extra mass to launch: your jumps leave the ground at {} speed.",
                pct(self.jump_scale)
            ));
        }
        s
    }

    /// The HUD's line, only while overloaded: what is wrong and what it does.
    pub fn hud_line(&self) -> Option<String> {
        if !self.overloaded {
            return None;
        }
        let load = format!(
            "Overloaded {:.0} / {:.0} kg",
            self.input.carried_kg,
            self.limit_here_kg.unwrap_or(0.0)
        );
        Some(match self.mode {
            CarryMode::Realistic => {
                format!("{load}: walking at {}, no jumping", pct(self.speed_factor))
            }
            CarryMode::Forgiving => format!("{load} (Forgiving: no slowdown)"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(carried: f32, capacity: f32, bonus: f32) -> CarryInput {
        CarryInput { carried_kg: carried, capacity_kg: capacity, bonus_kg: bonus, ..CarryInput::default() }
    }

    const MARS_G: f32 = 3.72;
    const MOON_G: f32 = 1.62;

    /// Rule 1 at 1 g: the limit is the inventory's own plus what is worn, the
    /// number the inventory system always computed and the page never showed.
    /// Red, against a stub `limit_here_kg` returning the old fixed Some(50.0):
    /// "assertion `left == right` failed: the backpack's 25 kg is part of the
    /// limit; left: Some(50.0), right: Some(75.0)".
    #[test]
    fn the_limit_is_capacity_plus_worn_gear_at_one_g() {
        let s = evaluate(load(0.0, 50.0, 25.0), ONE_G_M_S2, CarryMode::Forgiving);
        assert_eq!(s.limit_here_kg, Some(75.0), "the backpack's 25 kg is part of the limit");
        assert_eq!(s.tile_value(), "0.0 / 75.0 kg");
        assert!(s.tile_note().contains("plus 25 kg from what you wear"), "{}", s.tile_note());
    }

    /// Rule 1 away from 1 g: the same legs hold up 9.81 / g times the mass.
    /// Red, same stub: "Mars limit Some(50.0)".
    #[test]
    fn low_gravity_raises_the_limit_and_high_gravity_lowers_it() {
        let mars = limit_here_kg(75.0, MARS_G).unwrap();
        assert!((mars - 75.0 * 9.81 / 3.72).abs() < 0.01, "Mars limit {mars}");
        let moon = limit_here_kg(50.0, MOON_G).unwrap();
        assert!((moon - 302.8).abs() < 0.1, "Moon limit {moon}");
        let two_g = limit_here_kg(75.0, 2.0 * ONE_G_M_S2).unwrap();
        assert!((two_g - 37.5).abs() < 0.01, "2 g limit {two_g}");
        // The same 80 kg overloads a walker on Earth and not on Mars.
        assert!(evaluate(load(80.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Realistic).overloaded);
        assert!(!evaluate(load(80.0, 50.0, 0.0), MARS_G, CarryMode::Realistic).overloaded);
    }

    /// Rule 1 in zero g: no weight, no limit, never overloaded, and the page
    /// says so instead of printing an enormous number. Mass still applies to
    /// the jump (rule 3). Red, same stub: "weightless means no limit; left:
    /// Some(50.0), right: None".
    #[test]
    fn zero_g_has_no_limit_but_the_mass_is_still_there() {
        let s = evaluate(load(500.0, 50.0, 0.0), 0.01, CarryMode::Realistic);
        assert_eq!(s.limit_here_kg, None, "weightless means no limit");
        assert!(!s.overloaded);
        assert_eq!(s.speed_factor, 1.0);
        assert!(s.jump_scale > 0.0 && s.jump_scale < 0.4, "500 kg is still mass: {}", s.jump_scale);
        assert_eq!(s.tile_value(), "500.0 kg, weightless");
        assert!(s.tile_note().contains("weightless"), "{}", s.tile_note());
        // An unknown gravity never slows anyone.
        assert_eq!(limit_here_kg(50.0, f32::NAN), None);
    }

    /// Rule 2's slope: full speed to the limit, a tenth off per tenth over,
    /// never below a crawl. Red, against a stub returning 1.0: "10% over walks
    /// at 90%: 1".
    #[test]
    fn overloaded_walking_slows_by_how_far_over() {
        assert_eq!(walk_speed_factor(0.0), 1.0);
        assert_eq!(walk_speed_factor(1.0), 1.0);
        assert!((walk_speed_factor(1.1) - 0.9).abs() < 1e-5, "10% over walks at 90%: {}", walk_speed_factor(1.1));
        assert!((walk_speed_factor(1.5) - 0.5).abs() < 1e-5);
        assert_eq!(walk_speed_factor(3.0), CRAWL_SPEED, "never a dead stop");
        assert_eq!(walk_speed_factor(f32::INFINITY), CRAWL_SPEED);
        let mut last = 1.0;
        for i in 0..300 {
            let f = walk_speed_factor(1.0 + i as f32 * 0.01);
            assert!(f <= last, "heavier is never faster");
            last = f;
        }
    }

    /// Rule 2 whole: Realistic and 20% over the limit under 1 g walks at 80%
    /// and does not jump. Red, against a stub `evaluate` that only warned
    /// (speed 1, jump 1, the old do-nothing behaviour): "Realistic walks
    /// slower when over: 1".
    #[test]
    fn realistic_overload_walks_slower_and_cannot_jump() {
        let s = evaluate(load(60.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Realistic);
        assert!(s.overloaded);
        assert!((s.speed_factor - 0.8).abs() < 1e-5, "Realistic walks slower when over: {}", s.speed_factor);
        assert_eq!(s.jump_scale, 0.0, "no jumping overloaded under gravity");
        let note = s.tile_note();
        assert!(note.contains("walk at 80% speed") && note.contains("cannot jump"), "{note}");
        assert_eq!(s.hud_line().as_deref(), Some("Overloaded 60 / 50 kg: walking at 80%, no jumping"));
    }

    /// Rule 4: Forgiving keeps the limit and the warning and changes nothing
    /// about movement, and says where the other mode is. Red, against a stub
    /// `hud_line` returning None: "Forgiving still warns".
    #[test]
    fn forgiving_overload_only_warns() {
        let s = evaluate(load(60.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Forgiving);
        assert!(s.overloaded);
        assert_eq!((s.speed_factor, s.jump_scale), (1.0, 1.0));
        let hud = s.hud_line().expect("Forgiving still warns");
        assert_eq!(hud, "Overloaded 60 / 50 kg (Forgiving: no slowdown)");
        assert!(s.tile_note().contains(SETTING_PATH), "{}", s.tile_note());
        // Under the limit there is nothing to warn about.
        assert_eq!(evaluate(load(40.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Forgiving).hud_line(), None);
    }

    /// Rule 3: under the limit, Realistic still launches a loaded jump slower
    /// (the same push, more mass), in low gravity too: carry heavier, mass
    /// still applies. Red, against a stub `mass_jump_scale` returning 1.0:
    /// "30 kg on a 70 kg body launches at sqrt(0.7): 1".
    #[test]
    fn mass_still_applies_to_the_jump() {
        assert_eq!(mass_jump_scale(0.0), 1.0);
        let thirty = mass_jump_scale(30.0);
        assert!((thirty - 0.7f32.sqrt()).abs() < 1e-5, "30 kg on a 70 kg body launches at sqrt(0.7): {thirty}");
        let s = evaluate(load(30.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Realistic);
        assert!(!s.overloaded);
        assert_eq!(s.speed_factor, 1.0, "under the limit walks at full speed");
        assert!((s.jump_scale - thirty).abs() < 1e-6);
        assert!(s.tile_note().contains("leave the ground at 84% speed"), "{}", s.tile_note());
        // The Moon: 150 kg is half the limit, and still cuts the jump to 56%.
        let moon = evaluate(load(150.0, 50.0, 0.0), MOON_G, CarryMode::Realistic);
        assert!(!moon.overloaded);
        assert!((moon.jump_scale - 0.564).abs() < 0.001, "{}", moon.jump_scale);
        // Forgiving never touches the jump.
        assert_eq!(evaluate(load(30.0, 50.0, 0.0), ONE_G_M_S2, CarryMode::Forgiving).jump_scale, 1.0);
    }

    /// The default state (no player inventory yet) is the inventory's own
    /// default: nothing carried, 50 kg, 65 L, nothing to warn about. Red,
    /// against a derived all-zero default (the easy mistake, which would show
    /// "0.0 / 0.0 kg" before world entry): "assertion `left == right` failed;
    /// left: 0.0, right: 65.0" (the volume, the first field the stub left
    /// at zero that the fixed-50 stub limit did not mask).
    #[test]
    fn the_default_is_an_empty_inventory_at_one_g() {
        let s = CarryState::default();
        assert_eq!(s.tile_value(), "0.0 / 50.0 kg");
        assert_eq!(s.input.volume_capacity_l, 65.0);
        assert!(!s.overloaded && s.hud_line().is_none());
        assert_eq!((s.speed_factor, s.jump_scale), (1.0, 1.0));
    }
}
