//! A crop's path through one step, solved exactly (2026-10-30, docs/design/farm-clock.md rung 1).
//!
//! Until this rung FarmingSystem moved every crop once a tick at the values it found: the water
//! stress, the light and the health at one moment, held for the whole step. At a frame's step (a
//! sixtieth of a game second at normal speed) that cannot be seen; at a longer one it can, and a
//! 30 fps player grew a different garden from a 120 fps one. Four places did it
//! (docs/design/sim-frame-cost.md, F4):
//!
//! 1. The season health record (`health_seconds += health x dt`) counted the whole step at the
//!    health the crop ended it with.
//! 2. A crop drying out crosses the 0.2 stress line partway through a step; one regime was applied
//!    to all of it, so a long step either let it dry for free or stressed it from the start.
//! 3. A step that spans sunrise or sunset counted all of it lit or all of it dark.
//! 4. The feeder topped a unit up once per step, so over a long step the unit ran short in the
//!    middle (that one is fixed where the feeder runs, in `FarmingSystem::tick`).
//!
//! Here a step is cut where its regime changes, and each piece is solved exactly. Within a piece
//! the water level, the health and the growth age all move in straight lines (every rate in the
//! crop loop is a constant per game second), so:
//! - the light is `light_spans`: the step cut at sunrise, sunset and the grow lights' switch-off,
//!   which the clock knows in advance (the sun's arc, `solar::sun_factor`, runs 06:00 to 18:00 on
//!   the solar dial, and the lamps' timer from sunset for `LightingData::lamp_hours`);
//! - the water is `WaterPath`: dehydration, rain and the irrigation's hold are a line clamped
//!   between a floor and full, so it crosses the stress line at most once, at the level over the
//!   rate;
//! - the health moves toward its ceiling in a line (or down to zero under stress) and then holds,
//!   so the season record is a sum of trapezoids, exactly the integral;
//! - the stage is the running maximum of the effective progress (age x climate x health factor
//!   over the season, the tick's own rule: "only advance forward"), taken over the whole path, so a
//!   crop that crossed a stage boundary while healthy keeps it, as many short steps would have;
//! - a crop that dies, or ripens with no picking window (the loop then skips it), stops at that
//!   moment: its water, health and records are where they were when it stopped, not at the step's
//!   end.
//!
//! Float sums in a different order are not bit-equal, so "the same at any step" means within a
//! rounding tolerance; step_tests.rs states it and measures it on the golden garden.

use super::{HEALTH_DECAY_RATE, HEALTH_RECOVERY_RATE, WATER_STRESS_THRESHOLD};

/// Sunrise on the solar dial: `solar::sun_factor`'s arc starts here (lighting.rs's SUNRISE_H).
const SUNRISE_H: f64 = 6.0;
/// Sunset on the solar dial, where that arc ends.
const SUNSET_H: f64 = 18.0;
/// Below this health (of 100) growth no longer slows further: the tick's growth factor is
/// `max(health / 100, 0.1)`, so the path is cut here too.
const GROWTH_FACTOR_FLOOR_H: f64 = 10.0;

/// A marker on a crop sown at this tick's end (a Plant button, the showcase), so the crop loop
/// gives it none of the step (2026-10-30, farm-clock.md rung 1). Every command a tick handles is
/// the player's at the step's END; without this a crop sown during a long step was watered,
/// aged and recorded for the whole step it was not there for. Removed by the loop it marks, never
/// saved.
#[derive(Debug, Clone, Copy, Default)]
pub struct SownThisTick;

/// What lights a grow area: the sun, the grow lights' timer, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Sun,
    Lamps,
    Dark,
}

impl Light {
    /// Its slot in the per-light arrays (`CropPathIn::rate`, `climate`).
    pub fn index(self) -> usize {
        match self {
            Light::Sun => 0,
            Light::Lamps => 1,
            Light::Dark => 2,
        }
    }
}

/// One part of a step under one light, game seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSpan {
    pub secs: f64,
    pub light: Light,
}

/// The light at solar hour `h` (any real number; the dial repeats every 24) and the next hour,
/// after `h`, where it changes. The sun is up from SUNRISE_H to SUNSET_H; the lamps are on from
/// sunset for `lamp_h` hours (lighting.rs `lamps_on_at`, never by day); the rest is dark. Each
/// interval is closed at its start, open at its end, so a boundary belongs to what follows it.
fn light_and_next(h: f64, lamp_h: f64) -> (Light, f64) {
    let day = (h / 24.0).floor() * 24.0;
    let x = h - day;
    let light = if (SUNRISE_H..SUNSET_H).contains(&x) {
        Light::Sun
    } else if (x - SUNSET_H).rem_euclid(24.0) < lamp_h {
        Light::Lamps
    } else {
        Light::Dark
    };
    let lamp_off = (SUNSET_H + lamp_h).rem_euclid(24.0);
    let next = [SUNRISE_H, SUNSET_H, lamp_off, 24.0, 24.0 + SUNRISE_H]
        .into_iter()
        .filter(|b| *b > x)
        .fold(f64::INFINITY, f64::min);
    (light, day + next)
}

/// The light through the step that ENDED at solar hour `end_hour`, `step_s` game seconds long,
/// as spans in order from the step's start. `hours_per_s` is how fast the solar dial turns (24 a
/// calendar day: `24 / GameTime::seconds_per_day`) and `lamp_h` how long the grow lights' timer
/// runs after sunset (0 with no lighting data). The spans' seconds add up to `step_s` exactly
/// (the last takes what is left), and neighbours under the same light are merged. A step longer
/// than a day is simply many spans. Empty for a step of no time.
///
/// The end hour is the clock's own (`GameTime::solar_hour_at`, the field the tick always read),
/// so a test that ticks without moving the clock still sees the hour it set. A step of no time
/// (the clock held) is one span of no seconds under the light of that moment, so what reads the
/// light still sees it.
pub fn light_spans(end_hour: f64, step_s: f64, hours_per_s: f64, lamp_h: f64) -> Vec<LightSpan> {
    let mut out: Vec<LightSpan> = Vec::new();
    if !(hours_per_s > 0.0) || !end_hour.is_finite() {
        return out;
    }
    let lamp_h = lamp_h.clamp(0.0, 24.0 - (SUNSET_H - SUNRISE_H));
    if !(step_s > 0.0) {
        out.push(LightSpan { secs: 0.0, light: light_and_next(end_hour, lamp_h).0 });
        return out;
    }
    let mut h = end_hour - step_s * hours_per_s;
    let mut used = 0.0;
    while h < end_hour {
        let (light, next) = light_and_next(h, lamp_h);
        // A boundary closer than the hour's own rounding (a clock years from its start) is
        // passed rather than spun on: the rest of the step goes to this light.
        let stop = if next > h { next.min(end_hour) } else { end_hour };
        let secs = if stop >= end_hour { step_s - used } else { (stop - h) / hours_per_s };
        match out.last_mut() {
            Some(last) if last.light == light => last.secs += secs,
            _ => out.push(LightSpan { secs, light }),
        }
        used += secs;
        h = stop;
    }
    out
}

/// The same spans, merged by whether the sun is up: what the pests' conditions follow (a slug
/// feeds in the damp of the night, a row cover warms a field by day; the lamps make no
/// difference to either).
pub fn sun_spans(spans: &[LightSpan]) -> Vec<(f64, bool)> {
    let mut out: Vec<(f64, bool)> = Vec::new();
    for s in spans {
        let up = s.light == Light::Sun;
        match out.last_mut() {
            Some(last) if last.1 == up => last.0 += s.secs,
            _ => out.push((s.secs, up)),
        }
    }
    out
}

/// A crop's water level through a step: it falls at the dehydration rate, rises with rain, is
/// held up at the irrigation's level, and stays between empty and full. A straight line between
/// a floor and a cap, so it crosses the stress line (WATER_STRESS_THRESHOLD) at most once.
///
/// The tick's old order (dry, then irrigate up to the target, then add rain) is this line at a
/// frame's step; at a long one it was not: a field held at its target in a drizzle ended a step
/// one step's rain above it, every step.
#[derive(Debug, Clone, Copy)]
pub struct WaterPath {
    /// The level the step starts from: the crop's own, raised at once to the irrigation's hold.
    start: f64,
    /// Level per game second: rain's gain less the dehydration.
    slope: f64,
    /// The floor: the irrigation's hold, or empty.
    lo: f64,
    /// The cap: full.
    hi: f64,
    /// Whether the crop is water-stressed at the step's start (just after it).
    stressed_first: bool,
    /// The moment, seconds into the step, it crosses the stress line, if it does.
    flip_at: Option<f64>,
}

impl WaterPath {
    /// From the level `level`, at `slope` a game second, with the irrigation holding it at
    /// `held_at` (None: nothing waters it).
    pub fn new(level: f64, slope: f64, held_at: Option<f64>) -> Self {
        let lo = held_at.unwrap_or(0.0).max(0.0);
        let hi = lo.max(1.0);
        let start = level.max(lo).min(hi);
        let line = f64::from(WATER_STRESS_THRESHOLD);
        let (stressed_first, flip_at) = if slope < 0.0 {
            // Falling: stressed from the start below the line; else from the moment it reaches
            // the line, if the floor lets it get there.
            if start < line {
                (true, None)
            } else if lo < line {
                (false, Some((start - line) / -slope))
            } else {
                (false, None)
            }
        } else if slope > 0.0 && start < line {
            // Rising out of stress (rain on a dry field).
            (true, Some((line - start) / slope))
        } else {
            (start < line, None)
        };
        Self { start, slope, lo, hi, stressed_first, flip_at }
    }

    /// The level `t` seconds into the step.
    pub fn at(&self, t: f64) -> f64 {
        (self.start + self.slope * t).clamp(self.lo, self.hi)
    }

    /// Whether the crop is water-stressed just after `t` seconds into the step.
    pub fn stressed_on(&self, t: f64) -> bool {
        match self.flip_at {
            Some(f) if t >= f => !self.stressed_first,
            _ => self.stressed_first,
        }
    }

    /// The moment the regime flips, if it does.
    pub fn flip_at(&self) -> Option<f64> {
        self.flip_at
    }
}

/// What one crop's step needs (`walk`). Health is 0 to 100, the tick's scale; times are game
/// seconds.
#[derive(Debug, Clone)]
pub struct CropPathIn<'a> {
    /// The step's light, from its start (`light_spans`).
    pub spans: &'a [LightSpan],
    /// The crop's growth-clock rate under each light, by `Light::index` (`light_growth_rate`).
    pub rate: [f64; 3],
    /// Its climate factor under each light (an outdoor field's season and temperature; a row
    /// cover warms it by day and less by night, so the two can differ). 1 indoors.
    pub climate: [f64; 3],
    pub water: WaterPath,
    pub health: f64,
    /// The health its nutrients, pests, soil pH, air and weeds allow, held for the step.
    pub ceiling: f64,
    /// Its growth age at the step's start: the clock then, less its `planted_at`.
    pub age: f64,
    /// Its season, game seconds (plants.csv growth_days); 0 when no stage is tracked.
    pub growth_s: f64,
    /// It stops at ripeness: no picking window, so the loop skips it from its last stage on.
    pub stop_when_ripe: bool,
}

/// Where one crop's step left it. `alive_s` is the part of the step it grew through: all of it,
/// or up to the moment it died or ripened (and stopped).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CropPath {
    pub water: f64,
    pub health: f64,
    /// The integral of health / 100 over the part it grew through: what `health_seconds` gains.
    pub health_s: f64,
    pub alive_s: f64,
    pub died: bool,
    pub ripened: bool,
    /// Its growth age where it stopped (or at the step's end).
    pub age: f64,
    /// Seconds of it not water-stressed: the part it breathed out water (and, lit, took in CO2).
    pub unstressed_s: f64,
    /// The integral, over its unstressed part, of its light rate times its growth factor
    /// (`max(health / 100, 0.1)`): its photosynthesis through the step, in the units the tick's
    /// CO2 and O2 exchange uses per second.
    pub photo_s: f64,
    /// The highest growth progress (age x climate / season) the path reached: its uptake share.
    pub max_clock: f64,
    /// The highest effective progress (that times the growth factor): its stage.
    pub max_eff: f64,
}

/// The health's slope and where it stops, by regime: under water stress it falls at the decay
/// rate to zero; watered, it moves toward the ceiling (up at the recovery rate, down at the
/// nutrient decline rate) and holds there. The tick's three branches, as rates.
fn health_slope(h: f64, ceiling: f64, stressed: bool) -> (f64, f64) {
    if stressed {
        (if h > 0.0 { -f64::from(HEALTH_DECAY_RATE) } else { 0.0 }, 0.0)
    } else if h < ceiling {
        (f64::from(HEALTH_RECOVERY_RATE), ceiling)
    } else if h > ceiling {
        (-f64::from(super::soil::NUTRIENT_DECLINE_RATE), ceiling)
    } else {
        (0.0, ceiling)
    }
}

/// The tick's growth factor at health `h`.
fn growth_factor(h: f64) -> f64 {
    (h / 100.0).clamp(0.0, 1.0).max(0.1)
}

/// The growth age at the end of every span and the highest growth progress reached, for a crop
/// that lives the whole step: what the feeder must cover before the health is known (the draw
/// follows the growth clock, not the health, `soil::uptake_fraction`). `walk` computes both the
/// same way, so a crop that lives the step gets the same numbers from either.
pub fn clock_over(spans: &[LightSpan], rate: &[f64; 3], climate: &[f64; 3], age: f64, growth_s: f64) -> (f64, f64) {
    let mut a = age;
    let mut best = start_clock(spans, climate, age, growth_s);
    for s in spans {
        a += rate[s.light.index()] * s.secs;
        if growth_s > 0.0 {
            best = best.max(a * climate[s.light.index()] / growth_s);
        }
    }
    (a, best)
}

/// The growth progress at the step's start, under the step's first light: where the running
/// maximum starts (the previous step's end), so a step of no time still reads the crop as it is.
fn start_clock(spans: &[LightSpan], climate: &[f64; 3], age: f64, growth_s: f64) -> f64 {
    match spans.first() {
        Some(s) if growth_s > 0.0 => age * climate[s.light.index()] / growth_s,
        _ => f64::NEG_INFINITY,
    }
}

/// The largest of `(c0 + c1 t)(f0 + f1 t)` over `0..=len`, and where.
fn product_max(c0: f64, c1: f64, f0: f64, f1: f64, len: f64) -> (f64, f64) {
    let at = |t: f64| (c0 + c1 * t) * (f0 + f1 * t);
    let mut best = (at(0.0), 0.0);
    let end = at(len);
    if end >= best.0 {
        best = (end, len);
    }
    let a = c1 * f1;
    if a < 0.0 {
        let v = -(c0 * f1 + c1 * f0) / (2.0 * a);
        if v > 0.0 && v < len && at(v) > best.0 {
            best = (at(v), v);
        }
    }
    best
}

/// Walk one crop through its step (see the module comment): every piece between two changes of
/// regime solved in closed form.
pub fn walk(p: &CropPathIn) -> CropPath {
    let track = p.growth_s > 0.0;
    let mut t = 0.0;
    let mut h = p.health.clamp(0.0, 100.0);
    let start = start_clock(p.spans, &p.climate, p.age, p.growth_s);
    let mut out = CropPath { max_clock: start, max_eff: start * growth_factor(h), ..Default::default() };
    let mut age_span0 = p.age;
    // Where it stopped (died, or ripened with no picking window): the moment, its age there and
    // the climate factor of the light it stopped under.
    let mut stop: Option<(f64, f64, f64)> = None;
    'spans: for span in p.spans {
        let (rate, clim) = (p.rate[span.light.index()], p.climate[span.light.index()]);
        let span_t0 = t;
        let span_end = span_t0 + span.secs;
        let age_at = |tt: f64| age_span0 + rate * (tt - span_t0);
        let c1 = if track { rate * clim / p.growth_s } else { 0.0 };
        // One pass a piece. A span holds at most a stress crossing, a landing on the ceiling or
        // on zero, and the growth factor's floor, so a handful of passes; the bound only keeps a
        // float surprise from spinning (and the debug build says so).
        let mut passes = 0;
        while t < span_end {
            passes += 1;
            debug_assert!(passes <= 32, "a step piece did not advance (t {t}, span end {span_end})");
            if passes > 32 {
                break;
            }
            let stressed = p.water.stressed_on(t);
            if stressed && h <= 0.0 {
                out.died = true;
                stop = Some((t, age_at(t), clim));
                break 'spans;
            }
            let mut next = span_end;
            if let Some(f) = p.water.flip_at() {
                if f > t && f < next {
                    next = f;
                }
            }
            let (slope, target) = health_slope(h, p.ceiling, stressed);
            // Where the health is at `next`: landed on its target, cut at the growth factor's
            // floor, or partway along its line.
            let mut h_next = None;
            if slope != 0.0 {
                let reach = t + (target - h) / slope;
                if reach <= next {
                    next = reach.max(t);
                    h_next = Some(target);
                }
                if (h - GROWTH_FACTOR_FLOOR_H) * (target - GROWTH_FACTOR_FLOOR_H) < 0.0 {
                    let k = t + (GROWTH_FACTOR_FLOOR_H - h) / slope;
                    if k < next {
                        next = k.max(t);
                        h_next = Some(GROWTH_FACTOR_FLOOR_H);
                    }
                }
            }
            let mut len = next - t;
            let mut h_end = h_next.unwrap_or_else(|| (h + slope * len).clamp(0.0, 100.0));
            let f0 = growth_factor(h);
            let mut f_end = growth_factor(h_end);
            let mut ripe_here = false;
            // The stage: the running maximum of the effective progress over this piece, and the
            // moment it ripens, where a crop with no picking window stops.
            if track && len > 0.0 {
                let c0 = age_at(t) * clim / p.growth_s;
                let f1 = (f_end - f0) / len;
                let (best, at_best) = product_max(c0, c1, f0, f1, len);
                if p.stop_when_ripe && best >= 1.0 {
                    // The first moment it reaches 1. The product is a parabola, so where it is at
                    // least 1 within the piece is one interval holding `at_best`: bisect for its
                    // start.
                    let eff = |x: f64| (c0 + c1 * x) * (f0 + f1 * x);
                    let (mut lo, mut hi) = (0.0, at_best);
                    if eff(0.0) >= 1.0 {
                        hi = 0.0;
                    } else {
                        for _ in 0..80 {
                            let mid = 0.5 * (lo + hi);
                            if eff(mid) >= 1.0 {
                                hi = mid;
                            } else {
                                lo = mid;
                            }
                        }
                    }
                    h_end = h + (h_end - h) * (hi / len);
                    f_end = f0 + f1 * hi;
                    len = hi;
                    next = t + hi;
                    ripe_here = true;
                    out.max_eff = out.max_eff.max(eff(hi)).max(1.0);
                } else {
                    out.max_eff = out.max_eff.max(best);
                }
            }
            if len > 0.0 {
                out.health_s += len * (h + h_end) / 200.0;
                if !stressed {
                    out.unstressed_s += len;
                    out.photo_s += rate * len * 0.5 * (f0 + f_end);
                }
            }
            t = next;
            h = h_end;
            if ripe_here {
                out.ripened = true;
                stop = Some((t, age_at(t), clim));
                break 'spans;
            }
            if stressed && h_next == Some(0.0) {
                // It dried out and its health ran to zero: it died here.
                out.died = true;
                stop = Some((t, age_at(t), clim));
                break 'spans;
            }
        }
        // The span's end, exactly: the same age `clock_over` reaches, so a crop that lives the
        // step draws exactly what the feeder covered.
        age_span0 += rate * span.secs;
        t = span_end;
        if track {
            out.max_clock = out.max_clock.max(age_span0 * clim / p.growth_s);
        }
    }
    match stop {
        Some((at, age, clim)) => {
            if track {
                out.max_clock = out.max_clock.max(age * clim / p.growth_s);
            }
            out.age = age;
            out.alive_s = at;
        }
        None => {
            out.age = age_span0;
            out.alive_s = t;
        }
    }
    out.health = h;
    out.water = p.water.at(out.alive_s);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Game seconds in an hour, and the solar dial's speed on a 24-hour day.
    const H: f64 = 3600.0;
    const DIAL: f64 = 1.0 / 3600.0;
    /// The shipped lamps' timer: six hours after sunset (lighting.ron's 18 h photoperiod).
    const LAMPS: f64 = 6.0;

    /// Within a millionth: the crop rates are f32 constants (the stress line is 0.2 as an f32,
    /// 0.2000000030 as an f64), so a time worked out from them is right to about that.
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
    }

    fn spans(end_hour: f64, step_s: f64) -> Vec<LightSpan> {
        light_spans(end_hour, step_s, DIAL, LAMPS)
    }

    /// The step is cut where the light changes, in order, and nothing else: two hours from 05:00
    /// are an hour of dark and an hour of sun; 17:30 to 00:30 is half an hour of sun, the lamps'
    /// six hours and half an hour of dark; a whole day from midnight is six, twelve and six hours
    /// and then dark again. The seconds add up to the step exactly, and a step of no time is the
    /// moment's light.
    #[test]
    fn a_step_is_cut_at_sunrise_sunset_and_the_lamps() {
        let s = |v: &[(f64, Light)]| v.iter().map(|(h, l)| LightSpan { secs: h * H, light: *l }).collect::<Vec<_>>();
        assert_eq!(spans(7.0, 2.0 * H), s(&[(1.0, Light::Dark), (1.0, Light::Sun)]));
        assert_eq!(spans(0.5, 7.0 * H), s(&[(0.5, Light::Sun), (6.0, Light::Lamps), (0.5, Light::Dark)]));
        assert_eq!(spans(24.0, 24.0 * H), s(&[(6.0, Light::Dark), (12.0, Light::Sun), (6.0, Light::Lamps)]));
        let day = spans(13.25, 24.0 * H + 1234.5);
        let total: f64 = day.iter().map(|x| x.secs).sum();
        assert_eq!(total, 24.0 * H + 1234.5, "the spans make up the step exactly");
        assert_eq!(spans(12.0, 0.0), vec![LightSpan { secs: 0.0, light: Light::Sun }]);
        assert_eq!(spans(19.0, 0.0)[0].light, Light::Lamps);
        assert_eq!(light_spans(19.0, 0.0, DIAL, 0.0)[0].light, Light::Dark, "no lighting data, no lamps");
        assert_eq!(sun_spans(&spans(0.5, 7.0 * H)), vec![(0.5 * H, true), (6.5 * H, false)]);
    }

    /// The water line: drying from 0.5 crosses the stress line when the level over the rate says
    /// (0.3 of a level at 0.1 an hour is three hours); the irrigation holding it at 0.8 never lets
    /// it cross; holding it below the line lets it cross and then holds it there, stressed; rain
    /// on a dry field lifts it out of stress at its own moment.
    #[test]
    fn the_water_crosses_the_stress_line_once_where_the_line_says() {
        let r = 0.1 / H;
        let dry = WaterPath::new(0.5, -r, None);
        assert!(!dry.stressed_on(0.0));
        assert!(close(dry.flip_at().unwrap(), 3.0 * H));
        assert!(dry.stressed_on(3.0 * H + 1.0));
        assert_eq!(dry.at(100.0 * H), 0.0, "empty, not below");
        let held = WaterPath::new(0.4, -r, Some(0.8));
        assert_eq!(held.at(0.0), 0.8, "the irrigation tops it up at once");
        assert!(held.flip_at().is_none() && !held.stressed_on(10.0 * H));
        let low = WaterPath::new(0.4, -r, Some(0.15));
        assert!(close(low.flip_at().unwrap(), 2.0 * H));
        assert_eq!(low.at(10.0 * H), 0.15);
        let rain = WaterPath::new(0.05, 0.05 / H, None);
        assert!(rain.stressed_on(0.0) && !rain.stressed_on(3.0 * H + 1.0));
        assert!(close(rain.flip_at().unwrap(), 3.0 * H));
        assert_eq!(rain.at(100.0 * H), 1.0, "full, not above");
    }

    fn crop<'a>(spans: &'a [LightSpan], water: WaterPath, health: f64, ceiling: f64) -> CropPathIn<'a> {
        CropPathIn {
            spans,
            rate: [2.0, 1.0, 0.0],
            climate: [1.0; 3],
            water,
            health,
            ceiling,
            age: 0.0,
            growth_s: 30.0 * 24.0 * H,
            stop_when_ripe: true,
        }
    }

    /// The season record is the integral of the path, not the end health times the step: a crop
    /// at 50 recovering at 25 an hour toward a ceiling of 80 rises for 1.2 hours and then holds,
    /// so over two hours it records (50 + 80) / 2 x 1.2 h + 80 x 0.8 h, in hundredths. The old
    /// tick counted the whole step at its end health, 80 x 2 h, a tenth too much.
    #[test]
    fn the_season_record_is_the_integral_of_the_path() {
        let sp = spans(12.0, 2.0 * H);
        let p = walk(&crop(&sp, WaterPath::new(0.9, 0.0, None), 50.0, 80.0));
        let want = ((50.0 + 80.0) / 2.0 * 1.2 * H + 80.0 * 0.8 * H) / 100.0;
        assert!(close(p.health_s, want), "{} vs {want}", p.health_s);
        assert_eq!(p.health, 80.0);
        assert_eq!((p.alive_s, p.died), (2.0 * H, false));
    }

    /// A crop drying out lives its regimes in order: watered until the level reaches the line
    /// (two hours from 0.3 at 0.05 an hour), then stressed, losing its health in two hours, and
    /// dead four hours into a five-hour step: its record is two hours at full health and the two
    /// hours' slide, its water where it was when it died (0.1), and it neither breathed nor grew
    /// after.
    #[test]
    fn a_crop_drying_out_dies_at_its_moment_not_the_steps_end() {
        let sp = spans(17.0, 5.0 * H);
        let p = walk(&crop(&sp, WaterPath::new(0.3, -0.05 / H, None), 100.0, 100.0));
        assert!(p.died);
        assert!(close(p.alive_s, 4.0 * H), "died at {} h", p.alive_s / H);
        assert!(close(p.health_s, 3.0 * H), "two hours at 1 and two at 0.5 on average: {}", p.health_s / H);
        assert!(close(p.water, 0.1), "the level at death: {}", p.water);
        assert!(close(p.unstressed_s, 2.0 * H), "watered for its first two hours only");
        assert!(close(p.age, 2.0 * 4.0 * H), "lit the four hours it lived: {} h", p.age / H);
    }

    /// THE POINT OF THE MODULE: walking one long step, or the same time in short steps carried
    /// from one to the next, ends in the same place. Over a day from 05:20 (sunrise, sunset and
    /// the lamps' switch-off all inside), for a crop drying into stress from a recovering health
    /// and one held watered under a falling ceiling, in one step and in 60 s steps.
    #[test]
    fn one_long_step_ends_where_many_short_ones_do() {
        let start_h = 5.0 + 1234.5 / H;
        for (water0, slope, held, h0, ceiling) in
            [(0.9, -0.1 / H, None, 60.0, 100.0), (0.6, -0.1 / H, Some(0.8), 95.0, 40.0), (0.5, 0.0, None, 5.0, 90.0)]
        {
            let total = 24.0 * H;
            let end_h = start_h + 24.0;
            let one_sp = light_spans(end_h, total, DIAL, LAMPS);
            let base = CropPathIn {
                spans: &one_sp,
                rate: [2.0, 0.6, 0.0],
                climate: [1.0, 0.9, 0.9],
                water: WaterPath::new(water0, slope, held),
                health: h0,
                ceiling,
                age: 0.2 * 30.0 * 24.0 * H,
                growth_s: 30.0 * 24.0 * H,
                stop_when_ripe: false,
            };
            let one = walk(&base);
            // The same day in 1,440 steps, each starting where the last ended.
            let (mut w, mut h, mut age) = (water0, h0, base.age);
            let (mut hs, mut alive, mut best) = (0.0, 0.0, f64::NEG_INFINITY);
            let n = 1440;
            for k in 0..n {
                let step = total / n as f64;
                let sp = light_spans(start_h + (k + 1) as f64 * step / H, step, DIAL, LAMPS);
                let p = walk(&CropPathIn { spans: &sp, water: WaterPath::new(w, slope, held), health: h, age, ..base.clone() });
                hs += p.health_s;
                alive += p.alive_s;
                best = best.max(p.max_eff);
                w = p.water;
                h = p.health;
                age = p.age;
                if p.died {
                    break;
                }
            }
            assert_eq!(one.died, h <= 0.0, "both died or neither");
            assert!((one.water - w).abs() < 1e-9, "water {} vs {w}", one.water);
            assert!((one.health - h).abs() < 1e-6, "health {} vs {h}", one.health);
            assert!((one.health_s - hs).abs() < 1e-6 * hs.max(1.0), "record {} vs {hs}", one.health_s);
            assert!((one.alive_s - alive).abs() < 1e-6, "lived {} vs {alive}", one.alive_s);
            assert!((one.age - age).abs() < 1e-6 * age, "age {} vs {age}", one.age);
            assert!((one.max_eff - best).abs() < 1e-9, "stage progress {} vs {best}", one.max_eff);
        }
    }

    /// A crop with no picking window stops the moment it ripens, mid-step: its age, health and
    /// record are where they were then, as many short steps (each skipping it once ripe) leave it.
    #[test]
    fn a_crop_stops_where_it_ripens() {
        let sp = spans(14.0, 4.0 * H);
        // Two lit hours of growth short of ripe at full health: ripe one hour in.
        let growth_s = 30.0 * 24.0 * H;
        let mut c = crop(&sp, WaterPath::new(0.9, 0.0, None), 100.0, 100.0);
        c.age = growth_s - 2.0 * H;
        let p = walk(&c);
        assert!(p.ripened && !p.died);
        assert!(close(p.alive_s, H), "ripe at {} h", p.alive_s / H);
        assert!(close(p.age, growth_s));
        assert!(p.max_eff >= 1.0 && p.max_clock >= 1.0);
        c.stop_when_ripe = false;
        let bearing = walk(&c);
        assert!(!bearing.ripened && bearing.alive_s == 4.0 * H, "a bearing plant goes on");
    }
}

