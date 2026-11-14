//! The score: the composer, the voice pool, the reverb, the genre's recipe, the level and the
//! limiter, rendering into a plain stereo buffer (docs/design/music.md, section 4.2).
//!
//! The game sends a small `Copy` [`MusicContext`] (kira's latest-value channel, kira_sound.rs); the
//! score reads it at the start of each block, steers the composer with it (the mood, the genre or
//! blend, the tuning, applied at the next phrase) and moves its output gain toward the context's
//! with a 1.5 s time constant (so music fades rather than cuts). On top of that gain, the DUCK
//! (`MusicContext::duck_db`, docs/design/music.md section 8, rung 4): other sound wins, so the game
//! asks the score to sit some dB under itself (a chat video playing, the world behind a page) and
//! the score goes down quickly and comes back slowly ([`DUCK_DOWN_S`], [`DUCK_UP_S`]). Everything is made in
//! [`ScoreEngine::new`]; [`ScoreEngine::render`] allocates nothing (a counting-allocator test holds
//! it to that).

use crate::composer::{Composer, Sink, Steer};
use crate::dsp::{db_to_amp, Fdn, Lfo, Limiter, Smoother, Svf, SvfCoefs};
use crate::instrument::Instrument;
use crate::resolved::Resolved;
use crate::voice::{BlockParams, NoteOn, VoicePool};

/// The largest block rendered at once inside (kira asks for 128 frames; offline renders ask more).
const CHUNK: usize = 64;

/// What the game tells the score each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicContext {
    /// The output gain: master x music volume, 0 when the music should be silent here.
    pub gain: f32,
    /// A4 in hertz (432 by default).
    pub a4_hz: f32,
    /// The mood the place asks for (an index in the library's moods).
    pub mood: u16,
    /// The genre, the second genre of a blend, and the blend (0 = all `genre_a`).
    pub genre_a: u16,
    pub genre_b: u16,
    pub blend: f32,
    /// 0..1: how much is going on (layers join at their `min_intensity`).
    pub intensity: f32,
    /// How far under itself the score sits now, dB (0 = not ducked; negative, down): other sound
    /// wins (a chat video's sound, the world heard from a page). Moved toward with its own ramp.
    pub duck_db: f32,
    /// What the person changed on the Music page (crate::tune; the default changes nothing).
    pub tune: crate::tune::Tune,
    /// The Music page is open and reading the analyser (the spectrum and the role meters in the
    /// status): measured only then, so the score costs nothing extra the rest of the time.
    pub analyse: bool,
}

/// The spectrum's bands: octaves centred on 31.25 Hz times a power of two, 31 Hz to 16 kHz.
pub const BANDS: usize = 10;
/// A band's centre, Hz.
pub fn band_hz(k: usize) -> f32 {
    31.25 * (1u32 << k.min(BANDS - 1)) as f32
}

/// The duck's time constant going down, seconds: within 1 dB of a 12 dB duck in 0.3 s (docs/design/
/// music.md section 8: "drops ... over 300 ms").
pub const DUCK_DOWN_S: f32 = 0.08;
/// And coming back up: within 1 dB in about 1.5 s ("comes back over 2 s"; leaving the escape
/// menu "opens it back up over a second").
pub const DUCK_UP_S: f32 = 0.5;

impl Default for MusicContext {
    fn default() -> Self {
        MusicContext { gain: 1.0, a4_hz: crate::tuning::A4_DEFAULT_HZ, mood: 0, genre_a: 0, genre_b: 0, blend: 0.0, intensity: 0.3, duck_db: 0.0, tune: crate::tune::Tune::default(), analyse: false }
    }
}

/// What the score reports back (the Dev page, the hum's key, the tests).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MusicStatus {
    pub mood: u16,
    pub key_pc: i8,
    pub chord: u16,
    pub phrase: u64,
    pub bar: u32,
    /// Bars begun since the score started (the runtime takes over at the next bar line on a genre
    /// change, BUG-297).
    pub bars: u64,
    /// The piece playing (crate::piece): its seed, the section of its form (its letter + 1, 0 with
    /// no form), which time through the form, and the piece's phrase index now playing.
    pub piece_seed: u64,
    pub section: u8,
    pub chorus: u32,
    pub form_pos: u64,
    pub tempo_bpm: f32,
    pub voices: u16,
    pub seconds: f64,
    /// The output gain right now (after the fade).
    pub gain: f32,
    /// The duck right now, as an amplitude (1 = not ducked).
    pub duck: f32,
    /// The audio thread's cost: the mean and the worst block so far, microseconds (kira_sound.rs
    /// times each `process`; the offline renderer times its own).
    pub block_us_mean: f32,
    pub block_us_worst: f32,
    pub blocks: u64,
    /// The highest sample this score has sent, and how often the limiter acted.
    pub peak: f32,
    pub limited: u64,
    /// Where in the bar the music is, in beats (2.4: the third beat, 40% through), and the beats a
    /// bar has (the Music page's beat pulse).
    pub beat: f32,
    pub beats_per_bar: u8,
    /// With the context's `analyse` on (the Music page open): each octave band's level (RMS of the
    /// output through a band-pass, quick to rise and slow to fall), and each role's activity (the
    /// velocity of its last note, falling away over about a third of a second). Zero otherwise.
    pub bands: [f32; BANDS],
    pub roles: [f32; crate::tune::ROLES],
}

struct PoolSink<'a> {
    pool: &'a mut VoicePool,
    res: &'a Resolved,
    rate: f32,
    /// Each role's gain from the person's tune (0: muted, or another role soloed), and the role
    /// meters the notes push up.
    gains: [f32; crate::tune::ROLES],
    meter: &'a mut [f32; crate::tune::ROLES],
    /// A ducking drum's sidechain, when one hit during this call.
    duck: &'a mut Option<(u8, f32, f32, f32)>,
}

impl Sink for PoolSink<'_> {
    fn note_on(&mut self, mut note: NoteOn) {
        // A muted role's notes never reach a voice (the composer composed them all the same, so the
        // piece is unchanged); a role's level scales its notes.
        let role = note.role as usize;
        if role < crate::tune::ROLES {
            let g = self.gains[role];
            if g <= 0.0 {
                return;
            }
            note.level *= g;
            self.meter[role] = self.meter[role].max(note.vel.clamp(0.0, 1.0));
        }
        self.pool.note_on(note, &self.res.instruments, &self.res.banks, self.rate);
    }
    fn note_off(&mut self, tag: u32) {
        self.pool.note_off(tag, &self.res.instruments, self.rate);
    }
    fn duck(&mut self, roles: u8, depth_db: f32, attack_s: f32, release_s: f32) {
        *self.duck = Some((roles, depth_db, attack_s, release_s));
    }
}

/// The formants of the vowels the choir glides between (the classic soprano table published in the
/// Csound manual's formant appendix, "a" and "o"): frequency, gain (dB), bandwidth.
const VOWEL_A: [(f32, f32, f32); 5] = [(800.0, 0.0, 80.0), (1150.0, -6.0, 90.0), (2900.0, -32.0, 120.0), (3900.0, -20.0, 130.0), (4950.0, -50.0, 140.0)];
const VOWEL_O: [(f32, f32, f32); 5] = [(450.0, 0.0, 70.0), (800.0, -11.0, 80.0), (2830.0, -22.0, 100.0), (3800.0, -22.0, 130.0), (4950.0, -50.0, 135.0)];

/// The score.
pub struct ScoreEngine {
    res: Resolved,
    rate: f32,
    composer: Composer,
    pool: VoicePool,
    reverb: Fdn,
    reverb_t60: f32,
    limiter: Limiter,
    ctx: MusicContext,
    gain: Smoother,
    level: Smoother,
    /// The duck as an amplitude, and whether its ramp is the falling one.
    duck: Smoother,
    duck_down: bool,
    wow: Lfo,
    flutter: Lfo,
    tape_l: Svf,
    tape_r: Svf,
    dry_l: [f32; CHUNK],
    dry_r: [f32; CHUNK],
    send_l: [f32; CHUNK],
    send_r: [f32; CHUNK],
    status: MusicStatus,
    stopping: bool,
    /// The analyser (only while the context's `analyse` is on): a band-pass per octave band, and
    /// the role meters.
    an: [Svf; BANDS],
    an_c: [SvfCoefs; BANDS],
    meter: [f32; crate::tune::ROLES],
    /// The sidechain (a genre ducking roles under its kick): its roles, depth (dB), attack and
    /// release (s), and the seconds since its last trigger.
    side: (u8, f32, f32, f32),
    side_t: f64,
}

/// The analyser's band-pass filters at `rate` (octave bands: Q about 1.4).
fn band_coefs(rate: f32) -> [SvfCoefs; BANDS] {
    let mut c = [SvfCoefs::default(); BANDS];
    for (k, x) in c.iter_mut().enumerate() {
        *x = SvfCoefs::new(band_hz(k).min(rate * 0.45), 1.414, rate);
    }
    c
}

impl ScoreEngine {
    /// Rendering offline (an export, a test), not on the audio thread: a streamed recording's note
    /// waits for its stream rather than fading when the render runs faster than the disk
    /// (crate::stream). Never on the audio thread.
    pub fn set_offline(&mut self, offline: bool) {
        self.pool.offline = offline;
    }

    /// How many notes are streaming a recording now.
    pub fn streaming(&self) -> usize {
        self.pool.streaming()
    }

    /// A score over `res` at `rate` Hz with `seed`, starting in the context's mood and genre.
    pub fn new(res: Resolved, rate: f32, seed: u64, ctx: MusicContext) -> Self {
        let steer = steer_of(&ctx);
        let composer = Composer::new(rate, seed, steer);
        ScoreEngine {
            res,
            rate,
            composer,
            pool: VoicePool::new(seed ^ 0x5eed),
            reverb: Fdn::new(4.8, rate),
            reverb_t60: 4.8,
            limiter: Limiter::new(-1.0, rate),
            ctx,
            gain: Smoother::new(0.0, 1.5, rate),
            level: Smoother::new(0.0, 4.0, rate),
            duck: Smoother::new(db_to_amp(ctx.duck_db.min(0.0)), DUCK_UP_S, rate),
            duck_down: false,
            wow: Lfo::new(0.0),
            flutter: Lfo::new(0.37),
            tape_l: Svf::default(),
            tape_r: Svf::default(),
            dry_l: [0.0; CHUNK],
            dry_r: [0.0; CHUNK],
            send_l: [0.0; CHUNK],
            send_r: [0.0; CHUNK],
            status: MusicStatus::default(),
            stopping: false,
            side: (0, 0.0, 0.005, 0.2),
            side_t: f64::MAX,
            an: [Svf::default(); BANDS],
            an_c: band_coefs(rate),
            meter: [0.0; crate::tune::ROLES],
        }
    }

    /// The same piece as a score that played it, carried on from the piece's phrase `played` (the
    /// runtime's rebuild mid-piece: the data read again with reuse off, a pack installed).
    pub fn resume(res: Resolved, rate: f32, seed: u64, ctx: MusicContext, played: u64) -> Self {
        let mut s = Self::new(res, rate, seed, ctx);
        s.composer.resume_at(played);
        s
    }

    /// A new context from the game (read at the start of the next block).
    pub fn set_context(&mut self, ctx: MusicContext) {
        self.ctx = ctx;
        self.composer.steer = steer_of(&ctx);
    }

    /// The context the score is following.
    pub fn context(&self) -> MusicContext {
        self.ctx
    }

    /// The sound card's rate changed (kira reports it through each block's time step).
    pub fn set_rate(&mut self, rate: f32) {
        if (rate - self.rate).abs() < 0.5 || rate < 8000.0 {
            return;
        }
        self.rate = rate;
        self.composer.set_rate(rate);
        self.reverb.set_rate(rate);
        self.limiter = Limiter::new(-1.0, rate);
        self.an_c = band_coefs(rate);
        let (g, l, d) = (self.gain.value, self.level.value, self.duck.value);
        self.gain = Smoother::new(g, 1.5, rate);
        self.level = Smoother::new(l, 4.0, rate);
        self.duck = Smoother::new(d, if self.duck_down { DUCK_DOWN_S } else { DUCK_UP_S }, rate);
    }

    /// Fade out over about two seconds, then [`ScoreEngine::finished`] is true.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Faded out after a stop.
    pub fn finished(&self) -> bool {
        self.stopping && self.gain.value < 1e-4
    }

    /// The latest status.
    pub fn status(&self) -> MusicStatus {
        self.status
    }

    /// Record the audio thread's time for one block (the adapter and the renderer time `render`).
    pub fn note_block_time(&mut self, us: f32) {
        let s = &mut self.status;
        s.blocks += 1;
        s.block_us_mean += (us - s.block_us_mean) / s.blocks.min(10_000) as f32;
        s.block_us_worst = s.block_us_worst.max(us);
    }

    /// The instruments the score plays from (tests).
    pub fn instruments(&self) -> &[Instrument] {
        &self.res.instruments
    }

    /// The composer (tests and the Dev page read it).
    pub fn composer(&self) -> &Composer {
        &self.composer
    }

    /// The resolved library (tests).
    pub fn resolved(&self) -> &Resolved {
        &self.res
    }

    /// Render `out.len()` stereo frames, overwriting `out`.
    pub fn render(&mut self, out: &mut [[f32; 2]]) {
        let rate = self.rate;
        let target_gain = if self.stopping { 0.0 } else { self.ctx.gain.clamp(0.0, 2.0) };
        let duck_target = db_to_amp(self.ctx.duck_db.clamp(-60.0, 0.0));
        let down = duck_target < self.duck.value;
        if down != self.duck_down {
            self.duck_down = down;
            self.duck.set_time(if down { DUCK_DOWN_S } else { DUCK_UP_S }, rate);
        }
        let gains = self.ctx.tune.gains();
        let mut done = 0;
        while done < out.len() {
            {
                let mut hit = None;
                let mut sink = PoolSink { pool: &mut self.pool, res: &self.res, rate, gains, meter: &mut self.meter, duck: &mut hit };
                self.composer.process(&self.res, &mut sink);
                if let Some(s) = hit {
                    self.side = s;
                    self.side_t = 0.0;
                }
            }
            let n = (out.len() - done).min(self.composer.until_next()).min(CHUNK);
            self.dry_l[..n].fill(0.0);
            self.dry_r[..n].fill(0.0);
            self.send_l[..n].fill(0.0);
            self.send_r[..n].fill(0.0);
            let (recipe, level_db) = self.composer.mix(&self.res);
            let (t60, wet) = self.composer.reverb(&self.res);
            if (t60 - self.reverb_t60).abs() > 0.05 {
                // A new mood's hall: the decay moves (no allocation: only the line gains change).
                self.reverb_t60 = t60;
                self.reverb.set_t60(t60);
            }
            // The genre's tape wow and flutter: one pitch multiplier for every voice this chunk.
            let wow = if recipe.wow_cents > 0.0 { self.wow.advance(recipe.wow_hz, n, rate) * recipe.wow_cents } else { 0.0 };
            let flutter = if recipe.flutter_cents > 0.0 { self.flutter.advance(recipe.flutter_hz, n, rate) * recipe.flutter_cents } else { 0.0 };
            let pitch_mul = 2f32.powf((wow + flutter) / 1200.0);
            // The choir's vowel glides between "ah" and "oh" over about 40 s.
            let t = self.composer.seconds() as f32;
            let m = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * t / 40.0).cos();
            let mut vowel = [(SvfCoefs::default(), 0.0f32); 5];
            for k in 0..5 {
                let (fa, ga, ba) = VOWEL_A[k];
                let (fo, go, bo) = VOWEL_O[k];
                let f = fa + (fo - fa) * m;
                let bw = ba + (bo - ba) * m;
                let g = ga + (go - ga) * m;
                vowel[k] = (SvfCoefs::new(f, f / bw, rate), db_to_amp(g));
            }
            // The sidechain's gain at this chunk's start and end: down to its depth over the attack,
            // back up in a straight line (in dB) over the release.
            let (mask, depth, attack, release) = self.side;
            let side_gain = |t: f64| -> f32 {
                let a = attack.max(1e-4) as f64;
                let d = if t < a { t / a } else { (1.0 - (t - a) / release.max(1e-3) as f64).max(0.0) };
                db_to_amp(depth * d as f32)
            };
            let ducking = mask != 0 && self.side_t < (attack + release) as f64;
            let (duck_from, duck_to) = if ducking { (side_gain(self.side_t), side_gain(self.side_t + n as f64 / rate as f64)) } else { (1.0, 1.0) };
            if self.side_t < f64::MAX {
                self.side_t += n as f64 / rate as f64;
            }
            let (bar, beats) = self.composer.beat_clock();
            let params = BlockParams { rate, pitch_mul, beat_hz: self.composer.beat_hz(), vowel: &vowel, bar, beats, duck_mask: if ducking { mask } else { 0 }, duck_from, duck_to };
            self.pool.render(&self.res.instruments, &self.res.banks, &params, &mut self.dry_l, &mut self.dry_r, &mut self.send_l, &mut self.send_r, n);
            self.reverb.process(&self.send_l[..n], &self.send_r[..n], &mut self.dry_l[..n], &mut self.dry_r[..n], wet * recipe.reverb_scale);
            let tape = (recipe.lowpass_hz > 0.0).then(|| SvfCoefs::new(recipe.lowpass_hz, 0.6, rate));
            let sat = recipe.saturation;
            let level_target = db_to_amp(level_db);
            for i in 0..n {
                let (mut l, mut r) = (self.dry_l[i], self.dry_r[i]);
                if let Some(c) = &tape {
                    l = self.tape_l.lp(c, l);
                    r = self.tape_r.lp(c, r);
                }
                let g = self.gain.next(target_gain) * self.level.next(level_target) * self.duck.next(duck_target);
                l *= g;
                r *= g;
                if sat > 0.0 {
                    let k = 1.0 + sat * 2.0;
                    l = (l * k).tanh() / k;
                    r = (r * k).tanh() / k;
                }
                let (l, r) = self.limiter.tick(l, r);
                out[done + i] = [l, r];
                self.status.peak = self.status.peak.max(l.abs()).max(r.abs());
            }
            self.composer.advance(n);
            done += n;
        }
        let s = &mut self.status;
        s.mood = self.composer.mood() as u16;
        s.key_pc = self.composer.key_pc() as i8;
        s.chord = self.composer.chord() as u16;
        s.phrase = self.composer.phrases();
        s.bar = self.composer.bar();
        s.bars = self.composer.bars();
        s.piece_seed = self.composer.piece_seed();
        (s.section, s.chorus, s.form_pos) = self.composer.section();
        s.tempo_bpm = self.composer.tempo();
        s.voices = self.pool.active() as u16;
        s.seconds = self.composer.seconds();
        s.gain = self.gain.value;
        s.duck = self.duck.value;
        s.limited = self.limiter.engaged;
        s.beat = self.composer.beat_pos();
        s.beats_per_bar = self.composer.beats_per_bar().min(255) as u8;
        self.analyse(out);
    }

    /// The Music page's analyser over the block just rendered (nothing unless the context asks):
    /// each octave band's RMS, quick up and slow down, and the role meters falling away.
    fn analyse(&mut self, out: &[[f32; 2]]) {
        let s = &mut self.status;
        if !self.ctx.analyse || out.is_empty() {
            s.bands = [0.0; BANDS];
            s.roles = [0.0; crate::tune::ROLES];
            self.meter = [0.0; crate::tune::ROLES];
            return;
        }
        let mut acc = [0.0f32; BANDS];
        for f in out {
            let m = 0.5 * (f[0] + f[1]);
            for k in 0..BANDS {
                let y = self.an[k].bp(&self.an_c[k], m);
                acc[k] += y * y;
            }
        }
        let n = out.len() as f32;
        // Falling: about a quarter of a second for the bands, a third for the meters.
        let fall = (-n / (self.rate * 0.25)).exp();
        for k in 0..BANDS {
            let v = (acc[k] / n).sqrt();
            s.bands[k] = if v > s.bands[k] { v } else { s.bands[k] * fall + v * (1.0 - fall) };
        }
        let fall_m = (-n / (self.rate * 0.33)).exp();
        s.roles = self.meter;
        for m in self.meter.iter_mut() {
            *m *= fall_m;
        }
    }
}

/// The composer's steer from a score's context (the sheet's dry run steers it the same way).
pub(crate) fn steer_of(ctx: &MusicContext) -> Steer {
    Steer { mood: ctx.mood, genre_a: ctx.genre_a, genre_b: ctx.genre_b, blend: ctx.blend, intensity: ctx.intensity, a4: ctx.a4_hz, tune: ctx.tune }
}
