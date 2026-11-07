//! The ship's ambient bed: the hum, and the optional tones (docs/design/music.md, sections 10 and 11).
//!
//! The operator, 2026-11-06: "I was also thinking we could incorporate a subtle brown noise that acts
//! like the hum of the engines and the resonance of everything?" The hum is brown noise (a leaky
//! integral of white noise), high-passed at 24 Hz and low-passed near 420 Hz, with six soft
//! resonances tuned to the score's KEY ROOT and fifth at the chosen tuning (so the bed and the music
//! agree; they glide to a new key over about 8 s), a faint tonal line of two oscillators 0.07 Hz apart
//! (two engines slightly out of step), and a breathing over 27 and 41 seconds. It plays ABOARD only,
//! louder and darker near a hum source (data/audio/hum_sources.ron: the drive and the reactors at the
//! stern, per docs/design/mothership-master-plan.md), softer in small rooms, and every change is
//! approached with a 2.5 s time constant, so a doorway is a crossfade, never a step. Off the ship it
//! fades out over 3 s.
//!
//! What it does not claim (the finding, Part E4): brown noise has popular claims for focus and sleep
//! and no good evidence for them. The hum is there because a ship should sound like a ship.
//!
//! The tones layer: steady soft tones some people like (the "solfeggio" set or any frequency), OFF
//! by default, playing only while the music is off (the game decides; these fixed frequencies are not
//! notes of the score's scale, so under the music they would clash). No health claim: "Some people
//! find these tones calming; there is no good evidence that they heal."

use serde::{Deserialize, Serialize};

use crate::dsp::{db_to_amp, Lfo, Phasor, Smoother, Svf, SvfCoefs};
use crate::rng::Rng;
use crate::tuning::note_hz_f64;

/// One hum source: a drive, a reactor pair, life support fans. Positions are SHIP metres (the frame
/// of data/blueprints/ship_structure.ron: y up, the origin at plot p1's corner).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HumSource {
    pub id: String,
    /// What it is ("drive", "reactor", "fans"), for people reading the file and the Dev page.
    pub kind: String,
    pub position: (f64, f64, f64),
    /// How loud it is at 4 m, as power over the ship's everywhere-hum (16 is +12 dB there).
    pub strength: f32,
    /// How much it darkens the hum and lifts its lowest resonances near it, 0..1.
    #[serde(default = "one")]
    pub colour: f32,
}

fn one() -> f32 {
    1.0
}

/// The hum sources file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HumSources {
    pub sources: Vec<HumSource>,
    /// The most any sources together raise the hum, dB.
    #[serde(default = "default_max_boost")]
    pub max_boost_db: f32,
    /// What one room boundary between a source and the listener takes off, dB.
    #[serde(default = "default_wall_db")]
    pub wall_db: f32,
    /// The stressed, alarm-like layer during a hull breach.
    #[serde(default)]
    pub alarm: AlarmSpec,
}

/// The hum under stress (a hull breach, a meteor storm): the structure strains (the hum louder and
/// harsher, its resonances wavering) and a distant two-tone klaxon sounds, its notes tuned to the
/// score's key at the chosen tuning, so even the alarm is in tune at A = 432 Hz.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AlarmSpec {
    /// The klaxon's two notes, semitones above the hum's root (D3 in D: 19 and 24 are A4 and D5,
    /// 432.0 and 576.7 Hz at A = 432), alternating this many times a second.
    pub tones: (f32, f32),
    pub rate_hz: f32,
    /// The klaxon's level over the hum's calibration, dB, and the low-pass it is heard through (a
    /// wall or two away).
    pub level_db: f32,
    pub muffle_hz: f32,
    /// The hum at full alarm: louder by this (dB), its low-pass opened by this (Hz), its resonances
    /// wavering by this (cents) at this rate (Hz).
    pub stress_db: f32,
    pub open_hz: f32,
    pub waver_cents: f32,
    pub waver_hz: f32,
}

impl Default for AlarmSpec {
    fn default() -> Self {
        AlarmSpec { tones: (19.0, 24.0), rate_hz: 1.2, level_db: -10.0, muffle_hz: 1800.0, stress_db: 5.0, open_hz: 380.0, waver_cents: 25.0, waver_hz: 0.35 }
    }
}

fn default_max_boost() -> f32 {
    12.0
}
fn default_wall_db() -> f32 {
    6.0
}

impl Default for HumSources {
    fn default() -> Self {
        HumSources { sources: Vec::new(), max_boost_db: 12.0, wall_db: 6.0, alarm: AlarmSpec::default() }
    }
}

/// The hum's shape at a place: its level against the ship's everywhere-hum (dB) and how near a
/// source it is (0 far .. 1 right beside one; it darkens the hum).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HumPlace {
    pub level_db: f32,
    pub nearness: f32,
}

/// The hum at `listener` (ship metres). `walls` is how many room boundaries stand between the
/// listener and the sources (1 inside an enclosed room: the sources are all outside the rooms), and
/// `room_volume_m3` the room's volume (a cabin of about 25 m3 is 4 dB softer than an open hall).
/// Each source falls about 6 dB per doubling of distance past 4 m. Pure: the game calls it each
/// frame on its own thread, and a test walks it.
pub fn hum_place(sources: &HumSources, listener: (f64, f64, f64), walls: u32, room_volume_m3: Option<f32>) -> HumPlace {
    let wall = db_to_amp(-sources.wall_db * walls as f32).powi(2);
    let mut power = 0.0f32;
    let mut dark = 0.0f32;
    for s in &sources.sources {
        let (dx, dy, dz) = (s.position.0 - listener.0, s.position.1 - listener.1, s.position.2 - listener.2);
        let d = (dx * dx + dy * dy + dz * dz).sqrt().max(4.0) as f32;
        let p = s.strength.max(0.0) * (4.0 / d).powi(2) * wall;
        power += p;
        dark += p * s.colour.clamp(0.0, 1.0);
    }
    let boost = (10.0 * (1.0 + power).log10()).min(sources.max_boost_db);
    let room = match room_volume_m3 {
        Some(v) if v > 0.0 => {
            let t = ((v.log10() - 25f32.log10()) / (2000f32.log10() - 25f32.log10())).clamp(0.0, 1.0);
            -4.0 * (1.0 - t)
        }
        _ => 0.0,
    };
    HumPlace { level_db: boost + room, nearness: (dark / (1.0 + power)).clamp(0.0, 1.0) }
}

/// What the game tells the bed each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BedContext {
    /// Aboard the mothership (the hum plays only there).
    pub aboard: bool,
    /// The place's shape (`hum_place`).
    pub place: HumPlace,
    /// The score's key centre (a pitch class; D = 2 when no music has played).
    pub key_pc: i8,
    pub a4_hz: f32,
    /// The hum's gain: master x ambience volume x ship hum level, 0 with the hum off.
    pub gain: f32,
    /// The tone's frequency and gain (0 = off; the game sets it only while the music is off).
    pub tone_hz: f32,
    pub tone_gain: f32,
    /// The breach alarm, 0 (none) .. 1 (full): the hum strains and the klaxon sounds.
    pub alarm: f32,
}

impl Default for BedContext {
    fn default() -> Self {
        BedContext { aboard: true, place: HumPlace { level_db: 0.0, nearness: 0.0 }, key_pc: 2, a4_hz: crate::tuning::A4_DEFAULT_HZ, gain: 1.0, tone_hz: 528.0, tone_gain: 0.0, alarm: 0.0 }
    }
}

/// What the bed reports back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BedStatus {
    /// The hum's level now (after the smoothing), as an amplitude factor.
    pub level: f32,
    pub cutoff_hz: f32,
    /// The root its resonances sit on now (a fractional MIDI note while gliding).
    pub root_midi: f32,
    pub tone_level: f32,
    pub alarm_level: f32,
    pub block_us_mean: f32,
    pub block_us_worst: f32,
    pub blocks: u64,
    pub peak: f32,
}

/// The resonances: (semitones from the root, gain, Q). The root sits at D3 for the key of D: D1, D2,
/// A2, D3, A3, D4 (36.0, 72.1, 108.0, 144.2, 216.0 and 288.3 Hz at A = 432): the higher ones carry
/// the hum on speakers that cannot play 36 to 72 Hz (a television, a laptop).
const RES: [(f32, f32, f32); 6] = [(-24.0, 0.5, 10.0), (-12.0, 1.0, 16.0), (-5.0, 0.7, 18.0), (0.0, 0.55, 22.0), (7.0, 0.3, 24.0), (12.0, 0.18, 26.0)];
/// The hum's gain that puts it at the ambient category's loudness (data/audio/loudness.toml, -26 LUFS)
/// at gain 1 in an open space far from the sources (measured by `the_hum_sits_at_the_ambient_target`).
const HUM_CAL: f32 = 0.473;
/// The tone's amplitude at gain 1: about the ambient target for a sine in the middle of the range.
const TONE_CAL: f32 = 0.07;
const CHUNK: usize = 64;

/// The bed.
pub struct BedEngine {
    rate: f32,
    rng: Rng,
    ctx: BedContext,
    bl: f32,
    br: f32,
    hp_l: Svf,
    hp_r: Svf,
    lp_l: Svf,
    lp_r: Svf,
    res_l: [Svf; 6],
    res_r: [Svf; 6],
    tone: [Phasor; 2],
    level: Smoother,
    cutoff: Smoother,
    near: Smoother,
    root: Smoother,
    tone_gain: Smoother,
    tone_osc: Phasor,
    tone_hz: f32,
    breath_a: Lfo,
    breath_b: Lfo,
    n: u64,
    status: BedStatus,
    stopping: bool,
    alarm_spec: AlarmSpec,
    alarm: Smoother,
    waver: Lfo,
    /// Each klaxon tone's own phase, both always turning, so each tone is one steady sine switched
    /// on and off (no phase jump when the other tone hands back to it).
    klaxon_ph: [f32; 2],
    klaxon_t: f32,
    klaxon_lp: Svf,
}

impl BedEngine {
    /// A bed at `rate` Hz with `seed`, its alarm as `alarm` describes.
    pub fn with_alarm(rate: f32, seed: u64, ctx: BedContext, alarm: AlarmSpec) -> Self {
        let mut b = Self::new(rate, seed, ctx);
        b.alarm_spec = alarm;
        b
    }

    /// A bed at `rate` Hz with `seed` (the default alarm).
    pub fn new(rate: f32, seed: u64, ctx: BedContext) -> Self {
        let root = root_for(ctx.key_pc as i32, 50.0);
        let mut b = BedEngine {
            rate,
            rng: Rng::new(seed),
            ctx,
            bl: 0.0,
            br: 0.0,
            hp_l: Svf::default(),
            hp_r: Svf::default(),
            lp_l: Svf::default(),
            lp_r: Svf::default(),
            res_l: [Svf::default(); 6],
            res_r: [Svf::default(); 6],
            tone: [Phasor::default(); 2],
            level: Smoother::new(0.0, 2.5, rate),
            cutoff: Smoother::new(420.0, 2.5, rate),
            near: Smoother::new(0.0, 2.5, rate),
            root: Smoother::new(root, 2.7, rate),
            tone_gain: Smoother::new(0.0, 2.0, rate),
            tone_osc: Phasor::new(ctx.tone_hz.max(1.0), 1.0, 0.0, rate),
            tone_hz: ctx.tone_hz,
            breath_a: Lfo::new(0.0),
            breath_b: Lfo::new(0.16),
            n: 0,
            status: BedStatus::default(),
            stopping: false,
            alarm_spec: AlarmSpec::default(),
            alarm: Smoother::new(0.0, 0.6, rate),
            waver: Lfo::new(0.0),
            klaxon_ph: [0.0; 2],
            klaxon_t: 0.0,
            klaxon_lp: Svf::default(),
        };
        b.retune_line(root, ctx.a4_hz);
        b
    }

    fn retune_line(&mut self, root: f32, a4: f32) {
        let f = note_hz_f64(root as f64 - 12.0, a4 as f64) as f32;
        self.tone = [Phasor::new(f, 1.0, 0.0, self.rate), Phasor::new(f + 0.07, 1.0, 0.0, self.rate)];
    }

    /// A new context from the game.
    pub fn set_context(&mut self, ctx: BedContext) {
        self.ctx = ctx;
    }

    /// Fade out, then [`BedEngine::finished`].
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Faded out after a stop.
    pub fn finished(&self) -> bool {
        self.stopping && self.level.value < 1e-5 && self.tone_gain.value < 1e-5 && self.alarm.value < 1e-4
    }

    /// The latest status.
    pub fn status(&self) -> BedStatus {
        self.status
    }

    /// Record one block's time on the audio thread, microseconds.
    pub fn note_block_time(&mut self, us: f32) {
        let s = &mut self.status;
        s.blocks += 1;
        s.block_us_mean += (us - s.block_us_mean) / s.blocks.min(10_000) as f32;
        s.block_us_worst = s.block_us_worst.max(us);
    }

    /// A new rate (the sound card's).
    pub fn set_rate(&mut self, rate: f32) {
        if (rate - self.rate).abs() < 0.5 || rate < 8000.0 {
            return;
        }
        self.rate = rate;
        self.level.set_time(2.5, rate);
        self.cutoff.set_time(2.5, rate);
        self.near.set_time(2.5, rate);
        self.root.set_time(2.7, rate);
        self.tone_gain.set_time(2.0, rate);
        let r = self.root.value;
        self.retune_line(r, self.ctx.a4_hz);
        self.tone_osc = Phasor::new(self.tone_hz.max(1.0), 1.0, 0.0, rate);
    }

    /// Render `out.len()` frames, overwriting `out`.
    pub fn render(&mut self, out: &mut [[f32; 2]]) {
        let rate = self.rate;
        let c = self.ctx;
        // Silent off the ship (3 s fade: the 2.5 s smoother reaches -60 dB in about 17 s, so leaving
        // drops the target to nothing and the time constant shortens to 1 s).
        let audible = c.aboard && !self.stopping;
        let target = if audible { db_to_amp(c.place.level_db) * c.gain.max(0.0) * HUM_CAL } else { 0.0 };
        let alarm_target = if audible { c.alarm.clamp(0.0, 1.0) } else { 0.0 };
        let aspec = self.alarm_spec;
        let klaxon_lp = SvfCoefs::new(aspec.muffle_hz, 0.7, rate);
        self.level.set_time(if audible { 2.5 } else { 1.0 }, rate);
        let tone_target = if self.stopping { 0.0 } else { c.tone_gain.max(0.0) * TONE_CAL };
        if (c.tone_hz - self.tone_hz).abs() > 0.01 && c.tone_hz > 0.0 {
            self.tone_hz = c.tone_hz;
            let (re, im) = (self.tone_osc.re, self.tone_osc.im);
            self.tone_osc = Phasor::new(c.tone_hz.min(rate * 0.45), 1.0, 0.0, rate);
            // keep the phase so a change of tone never clicks
            self.tone_osc.re = re;
            self.tone_osc.im = im;
            self.tone_osc.renormalise(1.0);
        }
        let want_root = root_for(c.key_pc as i32, self.root.value);
        let mut done = 0;
        let hp = SvfCoefs::new(24.0, 0.6, rate);
        while done < out.len() {
            let n = (out.len() - done).min(CHUNK);
            // Block-rate parameters: the place's darkness, the root's glide, the breathing.
            let near = self.near.advance(c.place.nearness, n);
            let alarm = self.alarm.advance(alarm_target, n);
            let cutoff = self.cutoff.advance(420.0 - 170.0 * near + aspec.open_hz * alarm, n);
            let waver = if alarm > 1e-4 { self.waver.advance(aspec.waver_hz, n, rate) * aspec.waver_cents * alarm } else { 0.0 };
            let stress = db_to_amp(aspec.stress_db * alarm);
            let klaxon_amp = db_to_amp(aspec.level_db) * HUM_CAL * alarm * c.gain.max(0.0);
            let lp = SvfCoefs::new(cutoff, 0.6, rate);
            let root = self.root.advance(want_root, n);
            if (root - self.status.root_midi).abs() > 0.01 {
                let a4 = c.a4_hz as f64;
                let f = note_hz_f64(root as f64 - 12.0, a4) as f32;
                self.tone[0].retune(f, 0.0, rate);
                self.tone[1].retune(f + 0.07, 0.0, rate);
            }
            self.status.root_midi = root;
            // The klaxon's two notes, tuned to the hum's root at the chosen tuning (once a block).
            let klaxon_hz = [aspec.tones.0, aspec.tones.1].map(|semi| note_hz_f64((root + semi) as f64, c.a4_hz as f64) as f32);
            let mut rc = [(SvfCoefs::default(), 0.0f32); 6];
            for (k, &(semi, g, q)) in RES.iter().enumerate() {
                let f = note_hz_f64((root + semi + waver / 100.0 * if k % 2 == 0 { 1.0 } else { -0.7 }) as f64, c.a4_hz as f64) as f32;
                // Near a source the lowest two resonances rise.
                let lift = if k < 2 { 1.0 + 1.2 * near } else { 1.0 };
                rc[k] = (SvfCoefs::new(f, q, rate), g * lift * 2.2);
            }
            let ba = self.breath_a.advance(1.0 / 27.0, n, rate);
            let bb = self.breath_b.advance(1.0 / 41.0, n, rate);
            let breath = 1.0 + 0.16 * ba + 0.07 * bb;
            for i in 0..n {
                let lvl = self.level.next(target);
                let tg = self.tone_gain.next(tone_target);
                let (mut yl, mut yr) = (0.0, 0.0);
                if lvl > 1e-6 {
                    let common = self.rng.bipolar() * 0.5;
                    let wl = common * 0.75 + self.rng.bipolar() * 0.33;
                    let wr = common * 0.75 + self.rng.bipolar() * 0.33;
                    self.bl = self.bl * 0.9975 + wl * 0.05;
                    self.br = self.br * 0.9975 + wr * 0.05;
                    let xl = self.lp_l.lp(&lp, self.hp_l.hp(&hp, self.bl));
                    let xr = self.lp_r.lp(&lp, self.hp_r.hp(&hp, self.br));
                    let (mut al, mut ar) = (xl * 0.55, xr * 0.55);
                    for k in 0..6 {
                        let (ref cf, g) = rc[k];
                        al += self.res_l[k].bp(cf, xl) * g;
                        ar += self.res_r[k].bp(cf, xr) * g;
                    }
                    let line = (self.tone[0].tick() + self.tone[1].tick()) * 0.012;
                    yl = (al + line) * lvl * breath * stress;
                    yr = (ar + line) * lvl * breath * stress;
                } else {
                    // Keep the line's oscillators turning so it resumes without a jump.
                    self.tone[0].tick();
                    self.tone[1].tick();
                }
                if klaxon_amp > 1e-6 {
                    // Two tones alternating, each edge softened over 15 ms so it never clicks; a
                    // square-ish shape (a third harmonic) heard through the walls (the low-pass).
                    let half = 0.5 / aspec.rate_hz.max(0.1);
                    self.klaxon_t += 1.0 / rate;
                    if self.klaxon_t >= 2.0 * half {
                        self.klaxon_t -= 2.0 * half;
                    }
                    let hi = self.klaxon_t >= half;
                    let into = if hi { self.klaxon_t - half } else { self.klaxon_t };
                    let edge = (into / 0.015).min(1.0).min(((half - into) / 0.015).max(0.0));
                    for (j, f) in klaxon_hz.into_iter().enumerate() {
                        self.klaxon_ph[j] = (self.klaxon_ph[j] + f / rate).fract();
                    }
                    let w = 2.0 * std::f32::consts::PI * self.klaxon_ph[usize::from(hi)];
                    let k = self.klaxon_lp.lp(&klaxon_lp, (w.sin() + 0.3 * (3.0 * w).sin()) * edge) * klaxon_amp;
                    yl += k;
                    yr += k * 0.9;
                }
                if tg > 1e-6 {
                    let s = self.tone_osc.tick() * tg;
                    yl += s;
                    yr += s;
                }
                out[done + i] = [yl, yr];
                self.status.peak = self.status.peak.max(yl.abs()).max(yr.abs());
            }
            self.n += n as u64;
            if self.n % 4096 < n as u64 {
                self.tone[0].renormalise(1.0);
                self.tone[1].renormalise(1.0);
                self.tone_osc.renormalise(1.0);
            }
            done += n;
        }
        self.status.level = self.level.value;
        self.status.cutoff_hz = self.cutoff.value;
        self.status.tone_level = self.tone_gain.value;
        self.status.alarm_level = self.alarm.value;
    }
}

/// The resonance root for key pitch class `pc`: the note of that class nearest `near` (D3 = 50 to
/// start), so a key change glides the shorter way (at most a tritone).
fn root_for(pc: i32, near: f32) -> f32 {
    let near_i = near.round() as i32;
    let mut best = near_i;
    let mut best_d = i32::MAX;
    for m in (near_i - 6)..=(near_i + 6) {
        if (m - pc).rem_euclid(12) == 0 && (m - near_i).abs() < best_d {
            best = m;
            best_d = (m - near_i).abs();
        }
    }
    // Keep the root between B2 and A3 so the resonances stay in the hum's band whatever the keys do.
    while best < 47 {
        best += 12;
    }
    while best > 57 {
        best -= 12;
    }
    best as f32
}
