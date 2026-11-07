//! The composer (docs/design/music.md, section 4.2 and rung 3): improvises within a mood and a
//! style, sample-accurate, on the audio thread.
//!
//! It plans a PHRASE at a time (8 or 16 bars): the chords, walked along the mood's weighted chord
//! graph and ending on one of its cadences; which motif plays on each chord, transposed to a chord
//! tone, with a note moved or the rhythm loosened; whether the choir sings, whether the deep
//! drums' heartbeat pulses, which drum pattern each part plays. A phrase it played within the mood's
//! `no_repeat_s` (10 minutes by default) is re-planned, so the score never repeats a phrase in that
//! window (a test renders an hour and checks). Chords are VOICE-LED: each pad and choir voice moves
//! to the nearest chord tone, never further than a fourth. The mood and the style change only at a
//! phrase boundary, and the key only at a section's end (every fourth phrase), so nothing jumps.
//!
//! What it decides goes to a [`Sink`]: the score's sink starts and stops voices; a test's sink
//! records, so an hour is "played" in milliseconds with no sound at all.

use crate::mood::RoleKind;
use crate::resolved::{RMood, RRole, Resolved};
use crate::rng::Rng;
use crate::style::{step_offset_in_beat, HarmonyRule, Recipe};
use crate::tuning::{scale_step, Mode};
use crate::voice::NoteOn;

/// What the composer reports besides notes (for tests, the status and the Dev page).
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// A phrase began: its plan's hash, when, in which mood and key.
    Phrase { hash: u64, at_s: f64, mood: u16, key_pc: i32 },
    /// A chord began: which, its pitch classes as a 12-bit mask, how many beats it lasts.
    Chord { chord: u16, pcs: u16, at_s: f64, beats: f32 },
    /// A role's voicing changed (pads and choir): its notes, lowest first.
    Voicing { role: u8, notes: [i32; 8], n: u8, at_s: f64 },
    /// A motif began on a role.
    Motif { role: u8, first_note: i32, at_s: f64 },
    /// The intensity band changed at a bar line (0 = calm, the place's own mood).
    Band { band: u16, at_s: f64 },
}

/// Where the composer's decisions go.
pub trait Sink {
    fn note_on(&mut self, note: NoteOn);
    fn note_off(&mut self, tag: u32);
    fn event(&mut self, _e: Event) {}
}

/// What the game asks of the composer, read at the next phrase.
#[derive(Clone, Copy, Debug)]
pub struct Steer {
    pub mood: u16,
    pub style_a: u16,
    /// The second style of a blend, and how far toward it (0 = all `style_a`).
    pub style_b: u16,
    pub blend: f32,
    pub intensity: f32,
    pub a4: f32,
}

const PENDING: usize = 128;
const HISTORY: usize = 256;
const MAX_SLOTS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Pending {
    at: u64,
    note: Option<NoteOn>,
}

#[derive(Clone, Copy, Default)]
struct MotifPlan {
    /// 0 none, else the role index + 1.
    role: u8,
    motif: u8,
    start_halfbeats: u8,
    anchor: u8,
    vary: i8,
    loosen: bool,
    vel: f32,
}

#[derive(Clone, Copy, Default)]
struct Plan {
    n: usize,
    chords: [u16; MAX_SLOTS],
    bars: [u8; MAX_SLOTS],
    motifs: [MotifPlan; MAX_SLOTS],
    choir: [bool; MAX_SLOTS],
    pulse: bool,
    patterns: [u8; 8],
    hash: u64,
}

/// One role's effective setup for the phrase (the mood's, under the style's palette and blend).
#[derive(Clone, Copy, Debug)]
struct EffRole {
    inst: u16,
    low: i32,
    high: i32,
    voices: u8,
    density: f32,
    level: f32,
}

#[derive(Clone, Copy, Debug)]
struct EffDrum {
    style: u16,
    part: u8,
    weight: f32,
}

/// The style (or blend) as it applies to this phrase.
#[derive(Clone, Copy, Debug)]
struct Eff {
    roles: [Option<EffRole>; 8],
    beat_felt: bool,
    subdivision: u32,
    swing: f32,
    syncopation: f32,
    humanize_ms: f32,
    drums: [Option<EffDrum>; 8],
    pad_pattern: Option<u16>,
    bass_pattern: Option<u16>,
    harmony: HarmonyRule,
    pub recipe: Recipe,
    level_db: f32,
    tempo: (f32, f32),
}

#[derive(Clone, Copy, Default)]
struct Held {
    notes: [i32; 8],
    tags: [u32; 8],
    n: usize,
}

/// The composer's whole state: fixed-size, made once.
pub struct Composer {
    rate: f64,
    rng: Rng,
    pub steer: Steer,
    mood: usize,
    key_idx: usize,
    key_pc: i32,
    mode: Mode,
    eff: Option<Eff>,
    tempo: f32,
    // time
    pos: u64,
    beat_start: f64,
    beat_len: f64,
    next_step: f64,
    step_in_beat: u32,
    beat_in_bar: u32,
    bar_in_phrase: u32,
    bars_in_phrase: u32,
    phrase_count: u64,
    // plan
    plan: Plan,
    slot: usize,
    bars_into_chord: u32,
    chord: usize,
    chord_pcs: [i32; 8],
    chord_n: usize,
    // what is held
    pads: Held,
    pad_voicing: Held,
    choir: Held,
    choir_voicing: Held,
    bass: Held,
    drone: Held,
    texture: Held,
    drone_root: i32,
    pending: [Pending; PENDING],
    history: [(u64, f32); HISTORY],
    hist_w: usize,
    tag: u32,
    started: bool,
    /// The intensity band playing, and a change waiting for a bar line: (band, the bar it starts on).
    band: usize,
    pending_band: Option<(usize, u64)>,
    bars_total: u64,
}

/// FNV-1a over a few numbers (a phrase plan's fingerprint).
fn fnv(h: u64, x: u64) -> u64 {
    let mut h = h;
    for b in x.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

impl Composer {
    /// A composer at `rate` Hz with `seed`, steered by `steer` from its first phrase.
    pub fn new(rate: f32, seed: u64, steer: Steer) -> Self {
        Composer {
            rate: rate as f64,
            rng: Rng::new(seed),
            steer,
            mood: steer.mood as usize,
            key_idx: 0,
            key_pc: 2,
            mode: Mode::Lydian,
            eff: None,
            tempo: 60.0,
            pos: 0,
            beat_start: 0.0,
            beat_len: rate as f64,
            next_step: 0.0,
            step_in_beat: 0,
            beat_in_bar: 0,
            bar_in_phrase: 0,
            bars_in_phrase: 0,
            phrase_count: 0,
            plan: Plan::default(),
            slot: 0,
            bars_into_chord: 0,
            chord: 0,
            chord_pcs: [0; 8],
            chord_n: 0,
            pads: Held::default(),
            pad_voicing: Held::default(),
            choir: Held::default(),
            choir_voicing: Held::default(),
            bass: Held::default(),
            drone: Held::default(),
            texture: Held::default(),
            drone_root: -1,
            pending: [Pending::default(); PENDING],
            history: [(0, f32::NEG_INFINITY); HISTORY],
            hist_w: 0,
            tag: 1,
            started: false,
            band: 0,
            pending_band: None,
            bars_total: 0,
        }
    }

    /// A new rate (the sound card's): times already planned are kept in beats.
    pub fn set_rate(&mut self, rate: f32) {
        let k = rate as f64 / self.rate;
        if (k - 1.0).abs() < 1e-9 {
            return;
        }
        self.rate = rate as f64;
        self.beat_len *= k;
        self.beat_start *= k;
        self.next_step *= k;
        self.pos = (self.pos as f64 * k) as u64;
        for p in self.pending.iter_mut() {
            p.at = (p.at as f64 * k) as u64;
        }
    }

    /// Seconds composed so far.
    pub fn seconds(&self) -> f64 {
        self.pos as f64 / self.rate
    }

    /// The key centre now (the hum follows it).
    pub fn key_pc(&self) -> i32 {
        self.key_pc
    }

    /// The mode now.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The mood playing now.
    pub fn mood(&self) -> usize {
        self.mood
    }

    /// The chord playing now (its index in the mood).
    pub fn chord(&self) -> usize {
        self.chord
    }

    /// The tempo, bpm.
    pub fn tempo(&self) -> f32 {
        self.tempo
    }

    /// Phrases begun.
    pub fn phrases(&self) -> u64 {
        self.phrase_count
    }

    /// The intensity band playing (0 = calm).
    pub fn band(&self) -> usize {
        self.band
    }

    /// The bar within the phrase.
    pub fn bar(&self) -> u32 {
        self.bar_in_phrase
    }

    /// The recipe and level the style gives the mix this phrase.
    pub fn mix(&self, res: &Resolved) -> (Recipe, f32) {
        let mood_level = res.moods.get(self.mood).map(|m| m.level_db).unwrap_or(0.0);
        match &self.eff {
            Some(e) => (e.recipe, mood_level + e.level_db),
            None => (Recipe { reverb_scale: 1.0, ..Default::default() }, mood_level),
        }
    }

    /// The reverb the mood asks for: (t60, wet).
    pub fn reverb(&self, res: &Resolved) -> (f32, f32) {
        res.moods.get(self.mood).map(|m| (m.reverb_t60_s, m.reverb_wet)).unwrap_or((4.8, 0.55))
    }

    /// Beats per second now.
    pub fn beat_hz(&self) -> f32 {
        (self.rate / self.beat_len) as f32
    }

    /// Samples until the next thing to decide (at least 1).
    pub fn until_next(&self) -> usize {
        let mut t = (self.next_step - self.pos as f64).ceil().max(1.0);
        for p in self.pending.iter() {
            if p.note.is_some() {
                t = t.min((p.at.saturating_sub(self.pos)).max(1) as f64);
            }
        }
        t as usize
    }

    /// Move time on by `n` samples (what the score just rendered).
    pub fn advance(&mut self, n: usize) {
        self.pos += n as u64;
    }

    /// Decide everything due now.
    pub fn process(&mut self, res: &Resolved, sink: &mut dyn Sink) {
        if !self.started {
            self.started = true;
            self.next_step = self.pos as f64;
            self.beat_start = self.pos as f64;
        }
        for i in 0..PENDING {
            if let Some(n) = self.pending[i].note {
                if self.pending[i].at <= self.pos {
                    self.pending[i].note = None;
                    sink.note_on(n);
                }
            }
        }
        let mut guard = 0;
        while self.next_step <= self.pos as f64 + 0.5 && guard < 64 {
            guard += 1;
            self.on_step(res, sink);
        }
    }

    /// Run the composer alone for `seconds` (no sound): the tests' hour in milliseconds.
    pub fn run_dry(&mut self, res: &Resolved, seconds: f64, sink: &mut dyn Sink) {
        let end = (seconds * self.rate) as u64 + self.pos;
        while self.pos < end {
            self.process(res, sink);
            let n = self.until_next().min((end - self.pos) as usize).max(1);
            self.advance(n);
        }
    }

    fn next_tag(&mut self) -> u32 {
        self.tag = self.tag.wrapping_add(1).max(1);
        self.tag
    }

    fn schedule(&mut self, at: u64, note: NoteOn) {
        if let Some(p) = self.pending.iter_mut().find(|p| p.note.is_none()) {
            p.at = at;
            p.note = Some(note);
        }
    }

    fn eff(&self) -> Eff {
        self.eff.unwrap_or_else(|| Eff {
            roles: [None; 8],
            beat_felt: false,
            subdivision: 2,
            swing: 0.5,
            syncopation: 0.0,
            humanize_ms: 0.0,
            drums: [None; 8],
            pad_pattern: None,
            bass_pattern: None,
            harmony: HarmonyRule::default(),
            recipe: Recipe { reverb_scale: 1.0, ..Default::default() },
            level_db: 0.0,
            tempo: (60.0, 60.0),
        })
    }

    fn steps_per_bar(&self, res: &Resolved) -> u32 {
        let bpb = res.moods.get(self.mood).map(|m| m.beats_per_bar).unwrap_or(4);
        bpb * self.eff().subdivision.max(1)
    }

    fn on_step(&mut self, res: &Resolved, sink: &mut dyn Sink) {
        let Some(mood) = res.moods.get(self.mood) else {
            // No mood: keep time moving so the score still ends cleanly.
            self.next_step += self.beat_len;
            return;
        };
        let _ = mood;
        if self.step_in_beat == 0 && self.beat_in_bar == 0 {
            self.on_bar(res, sink);
        }
        let bpb = res.moods.get(self.mood).map(|m| m.beats_per_bar).unwrap_or(4);
        let eff = self.eff();
        let sub = eff.subdivision.max(1);
        let step_of_bar = self.beat_in_bar * sub + self.step_in_beat;
        let spb = self.steps_per_bar(res);
        let now = self.pos;
        let rate = self.rate;
        // Drums.
        if eff.beat_felt {
            for d in eff.drums.iter().flatten() {
                let Some(part) = drum_part(res, self.mood, d.style, d.part) else { continue };
                // A part joins at its intensity, live on the beat (the taiko's doubled ostinato).
                if part.patterns.is_empty() || self.steer.intensity < part.min_intensity {
                    continue;
                }
                let pat = &part.patterns[self.plan.patterns[d.part as usize % 8] as usize % part.patterns.len()];
                if pat.is_empty() {
                    continue;
                }
                let idx = (step_of_bar as usize * pat.len()) / spb as usize;
                let mut v = pat[idx.min(pat.len() - 1)];
                if eff.syncopation > 0.0 && self.rng.chance(eff.syncopation * 0.25) {
                    v = if v > 0.0 { v * 0.6 } else if self.step_in_beat % 2 == 1 { 0.3 } else { 0.0 };
                }
                if v > 0.0 {
                    let jitter = (self.rng.bipolar() * eff.humanize_ms * 0.001 * rate as f32) as i64;
                    let at = (now as i64 + jitter.max(0)) as u64;
                    let tag = self.next_tag();
                    let note = NoteOn { inst: part.inst, role: 255, midi: part.midi as f32, a4: self.steer.a4, vel: (v * part.velocity * d.weight).min(1.0), pan: 0.0, hold_s: 0.0, tag, level: 1.0 };
                    if at > now {
                        self.schedule(at, note);
                    } else {
                        sink.note_on(note);
                    }
                }
            }
        }
        // The pad restruck on the style's comp pattern.
        if let (Some(si), Some(pad)) = (eff.pad_pattern, eff.roles[RoleKind::Pad.index()]) {
            if let Some(pat) = pattern_src(res, self.mood, si, false).and_then(|(p, _)| p) {
                if !pat.is_empty() {
                    let v = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    if v > 0.0 {
                        for k in 0..self.pads.n {
                            sink.note_off(self.pads.tags[k]);
                        }
                        self.pads.n = 0;
                        for k in 0..self.pad_voicing.n {
                            let tag = self.next_tag();
                            let midi = self.pad_voicing.notes[k];
                            sink.note_on(NoteOn { inst: pad.inst, role: RoleKind::Pad.index() as u8, midi: midi as f32, a4: self.steer.a4, vel: 0.55 + 0.35 * v + self.rng.range(-0.05, 0.05), pan: 0.0, hold_s: 0.0, tag, level: pad.level });
                            self.pads.notes[k] = midi;
                            self.pads.tags[k] = tag;
                            self.pads.n = k + 1;
                        }
                    }
                }
            }
        }
        // The bass on the style's pattern.
        if let (Some(si), Some(bass)) = (eff.bass_pattern, eff.roles[RoleKind::Bass.index()]) {
            if let Some(pat) = pattern_src(res, self.mood, si, true).and_then(|(_, b)| b) {
                if !pat.is_empty() {
                    let c = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    let root = self.chord_root_pc(res);
                    let off = match c {
                        b'x' | b'X' => Some(0),
                        b'5' => Some(7),
                        b'o' => Some(12),
                        b'3' => Some(if self.chord_has(root, 3) { 3 } else { 4 }),
                        _ => None,
                    };
                    if let Some(off) = off {
                        for k in 0..self.bass.n {
                            sink.note_off(self.bass.tags[k]);
                        }
                        let target = nearest_pc_in(root + off, bass.low, bass.high, if self.bass.n > 0 { self.bass.notes[0] } else { (bass.low + bass.high) / 2 });
                        let tag = self.next_tag();
                        sink.note_on(NoteOn { inst: bass.inst, role: RoleKind::Bass.index() as u8, midi: target as f32, a4: self.steer.a4, vel: 0.8 + self.rng.range(-0.08, 0.08), pan: 0.0, hold_s: 0.0, tag, level: bass.level });
                        self.bass = Held { notes: [target, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
                    }
                }
            }
        }
        // Danger: on each beat, see whether the intensity has crossed into another band.
        if self.step_in_beat == 0 {
            self.check_intensity(res, sink, bpb);
        }
        // Advance the grid.
        self.step_in_beat += 1;
        if self.step_in_beat >= sub {
            self.step_in_beat = 0;
            self.beat_in_bar += 1;
            self.beat_start += self.beat_len;
            if self.beat_in_bar >= bpb {
                self.beat_in_bar = 0;
                self.bar_in_phrase += 1;
            }
        }
        self.next_step = self.beat_start + step_offset_in_beat(self.step_in_beat, sub, eff.swing) as f64 * self.beat_len;
    }

    fn chord_root_pc(&self, res: &Resolved) -> i32 {
        res.moods.get(self.mood).and_then(|m| m.chords.get(self.chord)).map(|c| (self.key_pc + c.root).rem_euclid(12)).unwrap_or(self.key_pc)
    }

    fn chord_has(&self, root: i32, interval: i32) -> bool {
        self.chord_pcs[..self.chord_n].contains(&((root + interval).rem_euclid(12)))
    }

    fn on_bar(&mut self, res: &Resolved, sink: &mut dyn Sink) {
        let bar = self.bars_total;
        self.bars_total += 1;
        // A band change waiting for this bar line: the danger mood (or the calm one) begins here,
        // on a new phrase.
        let switch = matches!(self.pending_band, Some((_, at)) if bar >= at);
        if switch {
            if let Some((b, _)) = self.pending_band.take() {
                self.band = b;
                sink.event(Event::Band { band: b as u16, at_s: self.seconds() });
            }
        }
        if !self.started_phrase() || self.bar_in_phrase >= self.bars_in_phrase || switch {
            self.start_phrase(res, sink);
        }
        // Chord changes inside the phrase.
        if self.bars_into_chord >= self.plan.bars[self.slot] as u32 && self.slot + 1 < self.plan.n {
            self.slot += 1;
            self.bars_into_chord = 0;
            let c = self.plan.chords[self.slot] as usize;
            self.change_chord(res, sink, c);
        }
        self.bars_into_chord += 1;
        // The heartbeat: deep drums every few bars in phrases that have it.
        let eff = self.eff();
        if self.plan.pulse {
            if let (Some(perc), Some(mood)) = (eff.roles[RoleKind::Percussion.index()], res.moods.get(self.mood)) {
                if let Some(pulse) = mood.roles[RoleKind::Percussion.index()].as_ref().and_then(|r| r.pulse.as_ref()) {
                    let every = pulse.every_bars.max(1);
                    if self.bar_in_phrase % every == 0 {
                        let root = nearest_pc_in(self.key_pc, perc.low, perc.high, perc.low);
                        for &(beat, vel) in pulse.hits.iter().take(8) {
                            let at = self.pos + (beat.max(0.0) as f64 * self.beat_len) as u64;
                            let tag = self.next_tag();
                            let v = (vel * self.rng.range(0.85, 1.0)).min(1.0);
                            let pan = self.rng.range(-0.1, 0.1);
                            self.schedule(at, NoteOn { inst: perc.inst, role: RoleKind::Percussion.index() as u8, midi: root as f32, a4: self.steer.a4, vel: v, pan, hold_s: 0.0, tag, level: perc.level });
                        }
                    }
                }
            }
        }
    }

    fn started_phrase(&self) -> bool {
        self.phrase_count > 0
    }

    fn resolve_eff(&mut self, res: &Resolved) -> Eff {
        let mood = &res.moods[self.mood];
        let a = res.styles.get(self.steer.style_a as usize);
        let b = if self.steer.blend > 0.001 { res.styles.get(self.steer.style_b as usize) } else { None };
        let t = if b.is_some() { self.steer.blend.clamp(0.0, 1.0) } else { 0.0 };
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        let mut roles: [Option<EffRole>; 8] = [None; 8];
        let base = |r: &RRole| EffRole { inst: r.inst, low: r.low, high: r.high, voices: r.voices, density: r.density, level: r.level };
        for k in 0..8 {
            let pick = |s: Option<&crate::resolved::RStyle>| -> Option<EffRole> {
                let off = s.is_some_and(|s| s.roles_off[k]);
                let mut r = match (&mood.roles[k], s.and_then(|s| s.roles_add[k].as_ref())) {
                    (_, Some(add)) => Some(base(add)),
                    (Some(m), None) if !off => Some(base(m)),
                    _ => None,
                }?;
                if let Some((inst, db)) = s.and_then(|s| s.palette[k]) {
                    r.inst = inst;
                    r.level *= crate::dsp::db_to_amp(db);
                }
                Some(r)
            };
            let ra = pick(a);
            let rb = if b.is_some() { pick(b) } else { None };
            roles[k] = if b.is_none() {
                ra
            } else {
                // Each role takes the second style's instrument with a chance equal to the blend;
                // a role one style lacks plays while the blend leans to the style that has it.
                match (ra, rb) {
                    (Some(x), Some(y)) => Some(if self.rng.chance(t) { y } else { x }),
                    (Some(x), None) => (t < 0.5).then_some(x),
                    (None, Some(y)) => (t >= 0.5).then_some(y),
                    (None, None) => None,
                }
            };
            if roles[k].is_some() && self.steer.intensity < mood.roles[k].as_ref().map(|m| m.min_intensity).unwrap_or(0.0) {
                roles[k] = None;
            }
        }
        let ga = a.map(|s| s.groove).unwrap_or_default();
        let gb = b.map(|s| s.groove).unwrap_or(ga);
        let tempo_a = a.and_then(|s| s.tempo).unwrap_or(mood.tempo);
        let tempo_b = b.and_then(|s| s.tempo).unwrap_or(tempo_a);
        let felt_a = a.and_then(|s| s.beat_felt).unwrap_or(mood.beat_felt);
        let felt_b = b.and_then(|s| s.beat_felt).unwrap_or(felt_a);
        let mut drums: [Option<EffDrum>; 8] = [None; 8];
        let mut k = 0;
        for (si, w) in [(self.steer.style_a, 1.0 - t), (self.steer.style_b, t)] {
            if w < 0.15 {
                continue;
            }
            if let Some(s) = res.styles.get(si as usize) {
                for p in 0..s.drums.len().min(8) {
                    if k < 8 {
                        drums[k] = Some(EffDrum { style: si, part: p as u8, weight: w });
                        k += 1;
                    }
                }
            }
            if b.is_none() {
                break;
            }
        }
        let dominant = if t >= 0.5 { self.steer.style_b } else { self.steer.style_a };
        let dom = res.styles.get(dominant as usize);
        let ra = a.map(|s| s.recipe).unwrap_or(Recipe { reverb_scale: 1.0, ..Default::default() });
        let rb = b.map(|s| s.recipe).unwrap_or(ra);
        let recipe = Recipe {
            wow_cents: lerp(ra.wow_cents, rb.wow_cents),
            wow_hz: lerp(ra.wow_hz, rb.wow_hz),
            flutter_cents: lerp(ra.flutter_cents, rb.flutter_cents),
            flutter_hz: lerp(ra.flutter_hz, rb.flutter_hz),
            lowpass_hz: match (ra.lowpass_hz > 0.0, rb.lowpass_hz > 0.0) {
                (true, true) => lerp(ra.lowpass_hz, rb.lowpass_hz),
                (true, false) => lerp(ra.lowpass_hz, 20000.0),
                (false, true) => lerp(20000.0, rb.lowpass_hz),
                (false, false) => 0.0,
            },
            saturation: lerp(ra.saturation, rb.saturation),
            reverb_scale: lerp(ra.reverb_scale, rb.reverb_scale),
        };
        let mut eff = Eff {
            roles,
            beat_felt: if t >= 0.5 { felt_b } else { felt_a },
            subdivision: if t >= 0.5 { gb.subdivision } else { ga.subdivision }.clamp(1, 8),
            swing: lerp(ga.swing, gb.swing),
            syncopation: lerp(ga.syncopation, gb.syncopation),
            humanize_ms: lerp(ga.humanize_ms, gb.humanize_ms),
            drums,
            pad_pattern: dom.and_then(|s| s.pad_pattern.as_ref()).map(|_| dominant),
            bass_pattern: dom.and_then(|s| s.bass_pattern.as_ref()).map(|_| dominant),
            harmony: dom.map(|s| s.harmony).unwrap_or_default(),
            recipe,
            level_db: lerp(a.map(|s| s.level_db).unwrap_or(0.0), b.map(|s| s.level_db).unwrap_or(0.0)),
            tempo: (lerp(tempo_a.0, tempo_b.0), lerp(tempo_a.1, tempo_b.1)),
        };
        // The mood's own drums and patterns: an urgent mood's always, a calm one's where the style has
        // none (a mood that wants a gentle pulse under a style that brings no rhythm).
        let style_has_drums = eff.drums.iter().any(|d| d.is_some());
        if !mood.drums.is_empty() && (mood.urgent || !style_has_drums) {
            eff.drums = [None; 8];
            for p in 0..mood.drums.len().min(8) {
                eff.drums[p] = Some(EffDrum { style: MOOD_SRC, part: p as u8, weight: 1.0 });
            }
            if let Some(g) = mood.groove.filter(|_| !style_has_drums || mood.urgent) {
                eff.subdivision = g.subdivision.clamp(1, 8);
                eff.swing = g.swing;
                eff.syncopation = g.syncopation;
                eff.humanize_ms = g.humanize_ms;
            }
        }
        if mood.pad_pattern.is_some() && (mood.urgent || eff.pad_pattern.is_none()) {
            eff.pad_pattern = Some(MOOD_SRC);
        }
        if mood.bass_pattern.is_some() && (mood.urgent || eff.bass_pattern.is_none()) {
            eff.bass_pattern = Some(MOOD_SRC);
        }
        if mood.urgent {
            // Danger plays as written in every style: the mood's roles, tempo, beat and groove; the
            // style keeps only its recipe (tape colour) and level.
            for k in 0..8 {
                eff.roles[k] = mood.roles[k].as_ref().filter(|m| self.steer.intensity >= m.min_intensity).map(base);
            }
            eff.tempo = mood.tempo;
            eff.beat_felt = mood.beat_felt;
            eff.harmony = HarmonyRule::default();
            let g = mood.groove.unwrap_or_default();
            eff.subdivision = g.subdivision.clamp(1, 8);
            eff.swing = g.swing;
            eff.syncopation = g.syncopation;
            eff.humanize_ms = g.humanize_ms;
        }
        eff
    }

    /// On a beat: has the intensity crossed into another band? Rising: a stinger on this beat, a
    /// riser building to the next bar line (the one after if this bar is nearly over), and the
    /// danger mood from that bar line. Falling: back at the next `fall_bars` boundary, quietly.
    fn check_intensity(&mut self, res: &Resolved, sink: &mut dyn Sink, bpb: u32) {
        let ri = &res.intensity;
        if ri.froms.len() < 2 || self.phrase_count == 0 {
            return;
        }
        let from_band = self.pending_band.map(|(b, _)| b).unwrap_or(self.band);
        let want = crate::intensity::band_for(&ri.froms, from_band, self.steer.intensity, ri.hysteresis);
        if want == from_band {
            return;
        }
        if want == self.band {
            // It came back before the bar line: nothing changes.
            self.pending_band = None;
            return;
        }
        let beats_left = bpb.saturating_sub(self.beat_in_bar);
        if want > self.band {
            let extra = if beats_left <= 1 { 1 } else { 0 };
            // bars_total counts the bar now playing (on_bar has run for it), so the next bar line is
            // bar number bars_total.
            let at = self.bars_total + extra;
            if self.pending_band.is_none_or(|(b, _)| b < want) {
                // The stinger: on this beat, now.
                for &(inst, midi, vel, beat) in ri.stinger.iter().take(8) {
                    let tag = self.next_tag();
                    let n = NoteOn { inst, role: 254, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, hold_s: 0.0, tag, level: 1.0 };
                    if beat <= 0.0 {
                        sink.note_on(n);
                    } else {
                        let at_s = self.pos + (beat as f64 * self.beat_len) as u64;
                        self.schedule(at_s, n);
                    }
                }
                // The riser, held to the bar line.
                if let Some((inst, midi, vel)) = ri.riser {
                    let beats = beats_left as f64 + extra as f64 * bpb as f64;
                    let tag = self.next_tag();
                    let hold = (beats * self.beat_len / self.rate) as f32;
                    sink.note_on(NoteOn { inst, role: 254, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, hold_s: hold.max(0.05), tag, level: 1.0 });
                }
            }
            self.pending_band = Some((want, at));
        } else {
            // Settle at the next boundary of fall_bars bars.
            let every = ri.fall_bars.max(1) as u64;
            let at = self.bars_total.div_ceil(every) * every;
            self.pending_band = Some((want, at.max(self.bars_total)));
        }
    }

    fn start_phrase(&mut self, res: &Resolved, sink: &mut dyn Sink) {
        let first = self.phrase_count == 0;
        let target = res.band_mood(self.band).unwrap_or(self.steer.mood as usize).min(res.moods.len().saturating_sub(1));
        let mood_changed = first || target != self.mood;
        let prev_key = self.key_pc;
        if mood_changed {
            self.mood = target;
            let m = &res.moods[self.mood];
            // The new mood's key nearest the old one around the circle of fifths (its home key on
            // the first phrase), so a move between moods is a step, not a leap.
            self.key_idx = if first {
                0
            } else {
                (0..m.keys.len()).min_by_key(|&i| fifths_distance(m.keys[i].0, prev_key)).unwrap_or(0)
            };
        } else if self.phrase_count % 4 == 0 {
            // A section's end: now and then the key moves to a related one (by the keys' weights).
            let m = &res.moods[self.mood];
            if m.keys.len() > 1 && self.rng.chance(0.3) {
                let cur = self.key_idx;
                let pick = self.rng.weighted(m.keys.iter().enumerate().map(|(i, k)| if i == cur { 0.0 } else { k.2 }));
                self.key_idx = pick;
            }
        }
        let m = &res.moods[self.mood];
        let (kpc, kmode, _) = m.keys[self.key_idx.min(m.keys.len() - 1)];
        self.key_pc = kpc;
        self.mode = kmode;
        let eff = self.resolve_eff(res);
        self.eff = Some(eff);
        // Tempo, chosen per phrase within the range.
        self.tempo = if res.moods[self.mood].tempo_follows_intensity {
            // Across the band: its slow end where the band starts, its fast end where the next begins.
            let froms = &res.intensity.froms;
            let lo = froms.get(self.band).copied().unwrap_or(0.0);
            let hi = froms.get(self.band + 1).copied().unwrap_or(1.0).max(lo + 1e-3);
            let x = ((self.steer.intensity - lo) / (hi - lo)).clamp(0.0, 1.0);
            eff.tempo.0 + (eff.tempo.1 - eff.tempo.0) * x
        } else {
            self.rng.range(eff.tempo.0, eff.tempo.1.max(eff.tempo.0))
        }
        .clamp(20.0, 240.0);
        self.beat_len = self.rate * 60.0 / self.tempo as f64;
        // The plan, re-made while it repeats one played within the window.
        let now_s = self.seconds() as f32;
        let window = m.no_repeat_s;
        let start_chord = if mood_changed { m.start } else { self.chord };
        let mut plan = self.make_plan(m, start_chord, !mood_changed, eff);
        for _ in 0..24 {
            let seen = self.history.iter().any(|&(h, t)| h == plan.hash && now_s - t < window);
            if !seen {
                break;
            }
            plan = self.make_plan(m, start_chord, !mood_changed, eff);
        }
        self.history[self.hist_w] = (plan.hash, now_s);
        self.hist_w = (self.hist_w + 1) % HISTORY;
        self.plan = plan;
        self.bars_in_phrase = (0..plan.n).map(|i| plan.bars[i] as u32).sum::<u32>().max(1);
        self.bar_in_phrase = 0;
        self.slot = 0;
        self.bars_into_chord = 0;
        self.phrase_count += 1;
        sink.event(Event::Phrase { hash: plan.hash, at_s: self.seconds(), mood: self.mood as u16, key_pc: self.key_pc });
        // The drone follows the key.
        let drone = eff.roles[RoleKind::Drone.index()];
        let want_root = drone.map(|d| nearest_pc_in(self.key_pc, d.low, d.high, d.low)).unwrap_or(-1);
        if want_root != self.drone_root {
            for k in 0..self.drone.n {
                sink.note_off(self.drone.tags[k]);
            }
            self.drone.n = 0;
            if let Some(d) = drone {
                let tag = self.next_tag();
                sink.note_on(NoteOn { inst: d.inst, role: RoleKind::Drone.index() as u8, midi: want_root as f32, a4: self.steer.a4, vel: 0.9, pan: 0.0, hold_s: 0.0, tag, level: d.level });
                self.drone = Held { notes: [want_root, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
            }
            self.drone_root = want_root;
        }
        // The texture runs under everything while the style has one.
        match (eff.roles[RoleKind::Texture.index()], self.texture.n) {
            (Some(t), 0) => {
                let tag = self.next_tag();
                let mid = (t.low + t.high) / 2;
                sink.note_on(NoteOn { inst: t.inst, role: RoleKind::Texture.index() as u8, midi: mid as f32, a4: self.steer.a4, vel: 0.9, pan: 0.0, hold_s: 0.0, tag, level: t.level });
                self.texture = Held { notes: [mid, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
            }
            (None, n) if n > 0 => {
                sink.note_off(self.texture.tags[0]);
                self.texture.n = 0;
            }
            _ => {}
        }
        // A phrase with no bass lets go of what the last one held.
        if eff.roles[RoleKind::Bass.index()].is_none() {
            for k in 0..self.bass.n {
                sink.note_off(self.bass.tags[k]);
            }
            self.bass.n = 0;
        }
        let c = self.plan.chords[0] as usize;
        self.change_chord(res, sink, c);
    }

    fn make_plan(&mut self, m: &RMood, start: usize, continuing: bool, eff: Eff) -> Plan {
        let mut p = Plan::default();
        let bars_total = m.phrase_bars[self.rng.int(0, m.phrase_bars.len() as i32 - 1) as usize].clamp(1, 64);
        let (lo, hi) = eff.harmony.chord_bars.unwrap_or(m.chord_bars);
        let (lo, hi) = (lo.max(1), hi.max(lo.max(1)));
        let mut cur = if continuing { self.walk(m, start, false) } else { start };
        let mut used = 0u32;
        while used < bars_total && p.n < MAX_SLOTS {
            let mut bars = self.rng.int(lo as i32, hi as i32) as u32;
            if used + bars > bars_total || p.n == MAX_SLOTS - 1 {
                bars = bars_total - used;
            }
            p.chords[p.n] = cur as u16;
            p.bars[p.n] = bars.min(255) as u8;
            p.n += 1;
            used += bars;
            if used < bars_total {
                let last = used + lo > bars_total || used + hi >= bars_total;
                cur = self.walk(m, cur, last);
            }
        }
        // Motifs, the choir, the pulse, the drum patterns.
        let dens = |r: RoleKind| eff.roles[r.index()].map(|x| x.density).unwrap_or(0.0);
        let (db, dm, dc, dp) = (dens(RoleKind::Bells), dens(RoleKind::Melody), dens(RoleKind::Choir), dens(RoleKind::Percussion));
        for i in 0..p.n {
            let mut mp = MotifPlan::default();
            if !m.motifs.is_empty() {
                let role = if eff.roles[RoleKind::Bells.index()].is_some() && self.rng.chance(db) {
                    Some(RoleKind::Bells)
                } else if eff.roles[RoleKind::Melody.index()].is_some() && self.rng.chance(dm) {
                    Some(RoleKind::Melody)
                } else {
                    None
                };
                if let Some(r) = role {
                    mp.role = r.index() as u8 + 1;
                    mp.motif = self.rng.int(0, m.motifs.len() as i32 - 1) as u8;
                    mp.start_halfbeats = self.rng.int(1, 6) as u8;
                    mp.anchor = self.rng.int(0, 3) as u8;
                    mp.vary = self.rng.int(-1, 1) as i8;
                    mp.loosen = self.rng.chance(0.3);
                    mp.vel = self.rng.range(0.6, 1.0);
                }
            }
            p.motifs[i] = mp;
            p.choir[i] = self.rng.chance(dc);
        }
        p.pulse = self.rng.chance(dp);
        for k in 0..8 {
            p.patterns[k] = self.rng.int(0, 7) as u8;
        }
        // The fingerprint of what will SOUND: the key, the chords and their lengths, the motifs and
        // their variations, the choir where there is one, the pulse where there are drums for it,
        // the patterns of the drum parts that play. (A choice nobody hears, a pattern for a part the
        // phrase does not have, would make two identical-sounding phrases look different.)
        let has = |r: RoleKind| eff.roles[r.index()].is_some();
        let drum_parts = eff.drums.iter().flatten().count().min(8);
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        h = fnv(h, self.mood as u64);
        h = fnv(h, self.key_pc as u64);
        for i in 0..p.n {
            let mp = p.motifs[i];
            h = fnv(h, p.chords[i] as u64 | (p.bars[i] as u64) << 16);
            h = fnv(h, mp.role as u64 | (mp.motif as u64) << 8 | (mp.start_halfbeats as u64) << 16 | (mp.anchor as u64) << 24 | ((mp.vary as u8) as u64) << 32 | (mp.loosen as u64) << 40);
            h = fnv(h, (p.choir[i] && has(RoleKind::Choir)) as u64);
        }
        h = fnv(h, (p.pulse && has(RoleKind::Percussion)) as u64);
        for k in 0..drum_parts {
            h = fnv(h, p.patterns[k] as u64);
        }
        p.hash = h;
        p
    }

    /// One step along the chord graph from `cur`; `cadence` asks for a phrase-ending chord.
    fn walk(&mut self, m: &RMood, cur: usize, cadence: bool) -> usize {
        let moves = &m.moves[cur.min(m.moves.len() - 1)];
        if cadence && moves.iter().any(|(c, w)| m.cadence[*c] && *w > 0.0) {
            let i = self.rng.weighted(moves.iter().map(|(c, w)| if m.cadence[*c] { *w } else { 0.0 }));
            return moves[i].0;
        }
        let i = self.rng.weighted(moves.iter().map(|(_, w)| *w));
        moves[i].0
    }

    fn change_chord(&mut self, res: &Resolved, sink: &mut dyn Sink, chord: usize) {
        let m = &res.moods[self.mood];
        let Some(c) = m.chords.get(chord) else { return };
        self.chord = chord;
        let eff = self.eff();
        // The chord's pitch classes under the style's harmony rules.
        let root_pc = (self.key_pc + c.root).rem_euclid(12);
        let mut pcs = [0i32; 8];
        let mut n = 0;
        let add = |pc: i32, pcs: &mut [i32; 8], n: &mut usize| {
            let pc = pc.rem_euclid(12);
            if !pcs[..*n].contains(&pc) && *n < 8 {
                pcs[*n] = pc;
                *n += 1;
            }
        };
        for &iv in c.notes.iter() {
            if eff.harmony.plain && iv > 12 {
                continue;
            }
            add(root_pc + iv, &mut pcs, &mut n);
        }
        if eff.harmony.sevenths && !c.notes.iter().any(|&iv| iv.rem_euclid(12) == 10 || iv.rem_euclid(12) == 11) {
            // The diatonic seventh: six scale steps above the chord's root in the key's mode.
            let seventh = scale_step(root_pc + 60, 6, self.key_pc, self.mode);
            add(seventh, &mut pcs, &mut n);
        }
        self.chord_pcs = pcs;
        self.chord_n = n;
        let beats = self.plan.bars[self.slot] as f32 * m.beats_per_bar as f32;
        let mask = pcs[..n].iter().fold(0u16, |acc, &p| acc | 1 << p);
        sink.event(Event::Chord { chord: chord as u16, pcs: mask, at_s: self.seconds(), beats });
        let at_s = self.seconds();
        // Pads: voice-led.
        if let Some(pad) = eff.roles[RoleKind::Pad.index()] {
            let prev = self.pad_voicing;
            let notes = voice_lead(&prev.notes[..prev.n], pad.voices as usize, &pcs[..n], root_pc, pad.low, pad.high);
            self.pad_voicing = Held { notes, tags: [0; 8], n: pad.voices as usize };
            sink.event(Event::Voicing { role: RoleKind::Pad.index() as u8, notes, n: pad.voices, at_s });
            if eff.pad_pattern.is_none() {
                let held = self.pads;
                let mut next = Held::default();
                for k in 0..pad.voices as usize {
                    let midi = notes[k];
                    if let Some(j) = (0..held.n).find(|&j| held.notes[j] == midi && !next.tags[..next.n].contains(&held.tags[j])) {
                        next.notes[next.n] = midi;
                        next.tags[next.n] = held.tags[j];
                    } else {
                        let tag = self.next_tag();
                        let vel = 0.85 + self.rng.range(-0.05, 0.05);
                        sink.note_on(NoteOn { inst: pad.inst, role: RoleKind::Pad.index() as u8, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, hold_s: 0.0, tag, level: pad.level * if midi < 48 { 0.6 } else { 1.0 } });
                        next.notes[next.n] = midi;
                        next.tags[next.n] = tag;
                    }
                    next.n += 1;
                }
                for j in 0..held.n {
                    if !next.tags[..next.n].contains(&held.tags[j]) {
                        sink.note_off(held.tags[j]);
                    }
                }
                self.pads = next;
            }
        } else {
            for k in 0..self.pads.n {
                sink.note_off(self.pads.tags[k]);
            }
            self.pads.n = 0;
            self.pad_voicing.n = 0;
        }
        // Choir: on the chords the plan gave it, upper chord tones, voice-led.
        let choir_on = self.plan.choir[self.slot.min(MAX_SLOTS - 1)];
        match eff.roles[RoleKind::Choir.index()] {
            Some(ch) if choir_on => {
                let prev = self.choir;
                let led = self.choir_voicing;
                let notes = voice_lead(&led.notes[..led.n], ch.voices as usize, &pcs[..n], root_pc, ch.low, ch.high);
                self.choir_voicing = Held { notes, tags: [0; 8], n: ch.voices as usize };
                sink.event(Event::Voicing { role: RoleKind::Choir.index() as u8, notes, n: ch.voices, at_s });
                let mut next = Held::default();
                for k in 0..ch.voices as usize {
                    let midi = notes[k];
                    if let Some(j) = (0..prev.n).find(|&j| prev.notes[j] == midi && !next.tags[..next.n].contains(&prev.tags[j])) {
                        next.tags[next.n] = prev.tags[j];
                    } else {
                        let tag = self.next_tag();
                        sink.note_on(NoteOn { inst: ch.inst, role: RoleKind::Choir.index() as u8, midi: midi as f32, a4: self.steer.a4, vel: 0.8, pan: 0.0, hold_s: 0.0, tag, level: ch.level });
                        next.tags[next.n] = tag;
                    }
                    next.notes[next.n] = midi;
                    next.n += 1;
                }
                for j in 0..prev.n {
                    if !next.tags[..next.n].contains(&prev.tags[j]) {
                        sink.note_off(prev.tags[j]);
                    }
                }
                self.choir = next;
            }
            _ => {
                // Silent this chord: what it held is let go; its voicing is remembered
                // (choir_voicing) so the next entry is led from it.
                for k in 0..self.choir.n {
                    sink.note_off(self.choir.tags[k]);
                }
                self.choir.n = 0;
            }
        }
        // Bass, held (a style's pattern plays it on its steps instead).
        if let (Some(b), None) = (eff.roles[RoleKind::Bass.index()], eff.bass_pattern) {
            let prev = if self.bass.n > 0 { self.bass.notes[0] } else { (b.low + b.high) / 2 };
            let target = nearest_within(prev, &[root_pc, (root_pc + 7) % 12], 5).unwrap_or_else(|| nearest_pc_in(root_pc, b.low, b.high, prev));
            if self.bass.n == 0 || self.bass.notes[0] != target {
                for k in 0..self.bass.n {
                    sink.note_off(self.bass.tags[k]);
                }
                let tag = self.next_tag();
                sink.note_on(NoteOn { inst: b.inst, role: RoleKind::Bass.index() as u8, midi: target as f32, a4: self.steer.a4, vel: 0.85, pan: 0.0, hold_s: 0.0, tag, level: b.level });
                self.bass = Held { notes: [target, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
            }
        }
        // A motif on this chord, if the plan put one here.
        let mp = self.plan.motifs[self.slot.min(MAX_SLOTS - 1)];
        if mp.role > 0 {
            let role_idx = (mp.role - 1) as usize;
            if let (Some(r), Some(motif)) = (eff.roles[role_idx], m.motifs.get(mp.motif as usize)) {
                // Anchor: one of the chord's tones placed in the role's register.
                let tone = pcs[(mp.anchor as usize) % n.max(1)];
                let anchor = nearest_pc_in(tone, r.low, (r.high - 7).max(r.low), (r.low + r.high) / 2);
                let scale = self.mode.pentatonic();
                let chord_beats = beats;
                let mut beat = mp.start_halfbeats as f32 * 0.5;
                let first = scale_step(anchor, motif.steps[0], self.key_pc, scale).min(r.high);
                sink.event(Event::Motif { role: role_idx as u8, first_note: first, at_s });
                for (k, (&st, &bt)) in motif.steps.iter().zip(motif.beats.iter()).enumerate() {
                    if beat >= chord_beats {
                        break;
                    }
                    let step = st + if k == motif.steps.len() - 1 { mp.vary as i32 } else { 0 };
                    let midi = scale_step(anchor, step, self.key_pc, scale).clamp(r.low, r.high);
                    let at = self.pos + self.beats_to_samples(beat, eff) as u64;
                    let tag = self.next_tag();
                    let vel = (mp.vel * self.rng.range(0.85, 1.0)).min(1.0);
                    let len = bt * if mp.loosen && k == 1 { 1.5 } else { 1.0 };
                    let pan = self.rng.range(-0.45, 0.45);
                    self.schedule(at, NoteOn { inst: r.inst, role: role_idx as u8, midi: midi as f32, a4: self.steer.a4, vel, pan, hold_s: (len * self.beat_len as f32 / self.rate as f32) * 0.95, tag, level: r.level });
                    beat += len;
                }
            }
        }
    }

    /// A position in beats from now as samples, with the style's swing on the off-steps.
    fn beats_to_samples(&self, beats: f32, eff: Eff) -> f64 {
        let whole = beats.floor();
        let frac = beats - whole;
        let sub = eff.subdivision.max(1);
        let steps = frac * sub as f32;
        let k = steps.round();
        let frac = if (steps - k).abs() < 1e-3 { step_offset_in_beat(k as u32 % sub, sub, eff.swing) } else { frac };
        (whole + frac) as f64 * self.beat_len
    }
}

/// A drum part's or a pattern's source meaning "the mood's own", not a style's.
const MOOD_SRC: u16 = u16::MAX;

/// A drum part from the mood (`MOOD_SRC`) or a style.
fn drum_part(res: &Resolved, mood: usize, style: u16, part: u8) -> Option<&crate::resolved::RDrum> {
    if style == MOOD_SRC {
        res.moods.get(mood)?.drums.get(part as usize)
    } else {
        res.styles.get(style as usize)?.drums.get(part as usize)
    }
}

/// The pad and bass patterns of the mood (`MOOD_SRC`) or a style.
#[allow(clippy::type_complexity)]
fn pattern_src(res: &Resolved, mood: usize, src: u16, _bass: bool) -> Option<(Option<&Vec<f32>>, Option<&Vec<u8>>)> {
    if src == MOOD_SRC {
        let m = res.moods.get(mood)?;
        Some((m.pad_pattern.as_ref(), m.bass_pattern.as_ref()))
    } else {
        let s = res.styles.get(src as usize)?;
        Some((s.pad_pattern.as_ref(), s.bass_pattern.as_ref()))
    }
}

/// Distance between two pitch classes around the circle of fifths (0..6).
fn fifths_distance(a: i32, b: i32) -> i32 {
    let pos = |pc: i32| (pc * 7).rem_euclid(12);
    let d = (pos(a) - pos(b)).rem_euclid(12);
    d.min(12 - d)
}

/// The note of pitch class `pc` in [lo, hi] nearest `near` (or nearest the range when none fits).
pub fn nearest_pc_in(pc: i32, lo: i32, hi: i32, near: i32) -> i32 {
    let mut best = None;
    let mut m = lo;
    while m <= hi.max(lo) {
        if (m - pc).rem_euclid(12) == 0 && best.is_none_or(|b: i32| (m - near).abs() < (b - near).abs()) {
            best = Some(m);
        }
        m += 1;
    }
    best.unwrap_or_else(|| {
        let base = lo - (lo - pc).rem_euclid(12);
        if base < lo {
            base + 12
        } else {
            base
        }
    })
}

/// The nearest note to `from` whose pitch class is one of `pcs`, at most `max` semitones away.
fn nearest_within(from: i32, pcs: &[i32], max: i32) -> Option<i32> {
    let mut best: Option<i32> = None;
    for d in 0..=max {
        for m in [from - d, from + d] {
            if pcs.contains(&m.rem_euclid(12)) && best.is_none() {
                best = Some(m);
            }
        }
        if best.is_some() {
            break;
        }
    }
    best
}

/// Voice leading: the new voicing of `n` voices for the chord's pitch classes, each voice moving to
/// a chord tone at most a fourth (5 semitones) from where it was, choosing the set with the least
/// movement that keeps the chord's tones covered, doubles little and stays in [lo, hi]. With no
/// previous voicing (or a different voice count) it spreads the chord over the register.
pub fn voice_lead(prev: &[i32], n: usize, pcs: &[i32], root_pc: i32, lo: i32, hi: i32) -> [i32; 8] {
    let n = n.clamp(1, 8);
    let mut out = [0i32; 8];
    if pcs.is_empty() {
        return out;
    }
    if prev.len() != n {
        // Spread: evenly spaced targets, each snapped to the nearest chord tone, the root lowest.
        let span = (hi - lo).max(1) as f32;
        for (k, o) in out.iter_mut().take(n).enumerate() {
            let target = lo as f32 + span * (k as f32 + 0.5) / n as f32;
            let want = if k == 0 { &[root_pc][..] } else { pcs };
            *o = nearest_within(target.round() as i32, want, 6).unwrap_or(target.round() as i32);
        }
        out[..n].sort_unstable();
        return out;
    }
    let mut sorted = [0i32; 8];
    sorted[..n].copy_from_slice(&prev[..n]);
    sorted[..n].sort_unstable();
    // Candidates within a fourth for each voice (at most 11 notes, about 5 of them chord tones).
    let mut cand = [[0i32; 11]; 8];
    let mut cn = [0usize; 8];
    for v in 0..n {
        for d in -5..=5 {
            let m = sorted[v] + d;
            if pcs.contains(&m.rem_euclid(12)) && cn[v] < 11 {
                cand[v][cn[v]] = m;
                cn[v] += 1;
            }
        }
        if cn[v] == 0 {
            cand[v][0] = sorted[v];
            cn[v] = 1;
        }
    }
    let mut best_cost = i32::MAX;
    let mut best = sorted;
    let mut idx = [0usize; 8];
    let mut cur = [0i32; 8];
    loop {
        for v in 0..n {
            cur[v] = cand[v][idx[v]];
        }
        let mut cost = 0;
        for v in 0..n {
            cost += (cur[v] - sorted[v]).abs() * 2;
            if cur[v] < lo {
                cost += (lo - cur[v]) * 6;
            }
            if cur[v] > hi {
                cost += (cur[v] - hi) * 6;
            }
            for w in 0..v {
                if cur[w] == cur[v] {
                    cost += 20;
                } else if (cur[w] - cur[v]).abs() < 2 && cur[v] < 60 {
                    cost += 6; // a low cluster is mud
                }
            }
        }
        // Cover the chord: the root, its third or second, and its colour tones as far as the voices allow.
        for (i, &pc) in pcs.iter().enumerate() {
            if i >= n {
                break;
            }
            if !cur[..n].iter().any(|m| m.rem_euclid(12) == pc) {
                cost += if i < 3 { 14 } else { 7 };
            }
        }
        if cost < best_cost {
            best_cost = cost;
            best = cur;
        }
        // next combination
        let mut v = 0;
        loop {
            if v == n {
                let mut o = best;
                o[..n].sort_unstable();
                return o;
            }
            idx[v] += 1;
            if idx[v] < cn[v] {
                break;
            }
            idx[v] = 0;
            v += 1;
        }
    }
}
