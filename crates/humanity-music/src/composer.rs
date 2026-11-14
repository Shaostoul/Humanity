//! The composer (docs/design/music.md, section 4.2 and rung 3): improvises within a mood and a
//! genre, sample-accurate, on the audio thread.
//!
//! It plans a PHRASE at a time (8 or 16 bars): the chords, walked along the mood's weighted chord
//! graph and ending on one of its cadences; which motif plays on each chord, transposed to a chord
//! tone, with a note moved or the rhythm loosened; whether the choir sings, whether the deep
//! drums' heartbeat pulses, which drum pattern each part plays. A phrase it played within the mood's
//! `no_repeat_s` (10 minutes by default) is re-planned, so the score never repeats a phrase in that
//! window (a test renders an hour and checks). Chords are VOICE-LED: each pad and choir voice moves
//! to the nearest chord tone, never further than a fourth. The mood and the genre change only at a
//! phrase boundary, and the key only at a section's end (every fourth phrase), so nothing jumps.
//!
//! What it decides goes to a [`Sink`]: the score's sink starts and stops voices; a test's sink
//! records, so an hour is "played" in milliseconds with no sound at all.

use crate::genre::{step_offset_in_beat, Comping, HarmonyRule, Recipe};
use crate::mood::{Motif, RoleKind};
use crate::piece::{GenMotif, Piece, Section, MAX_PIECE_MOTIFS};
use crate::resolved::{RHarmony, RMood, RRole, Resolved};
use crate::rng::Rng;
use crate::tuning::{scale_step, Mode};
use crate::voice::NoteOn;

/// What the composer reports besides notes (for tests, the status and the Dev page).
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// A phrase began: its plan's hash, when, in which mood and key; the piece's section it plays
    /// (its letter + 1, 0 with no form), which time through the form, and whether it is a section
    /// RETURNING the first time through (its tune again, on purpose).
    Phrase { hash: u64, at_s: f64, mood: u16, key_pc: i32, section: u8, chorus: u32, returning: bool },
    /// A chord began: which, its root's pitch class, its pitch classes as a 12-bit mask, how many
    /// beats it lasts (the Music page names it from the root and the mask: "Dm6", "A7").
    Chord { chord: u16, root: i8, pcs: u16, at_s: f64, beats: f32 },
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
    pub genre_a: u16,
    /// The second genre of a blend, and how far toward it (0 = all `genre_a`).
    pub genre_b: u16,
    pub blend: f32,
    pub intensity: f32,
    pub a4: f32,
    /// What the person changed on the Music page (crate::tune): the key, the tempo, the swing and
    /// the lead voices are read here when a phrase is planned; the levels and mutes act in the score.
    pub tune: crate::tune::Tune,
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
    /// The phrase's dynamic level (velocity times this): a genre's `dynamics` range, else 1.
    vel_scale: f32,
}

/// One role's effective setup for the phrase (the mood's, under the genre's palette and blend).
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
    genre: u16,
    part: u8,
    weight: f32,
}

/// The genre (or blend) as it applies to this phrase.
#[derive(Clone, Copy, Debug)]
struct Eff {
    roles: [Option<EffRole>; 8],
    beat_felt: bool,
    subdivision: u32,
    swing: f32,
    syncopation: f32,
    humanize_ms: f32,
    drums: [Option<EffDrum>; 8],
    pad_patterns: Option<u16>,
    bass_patterns: Option<u16>,
    stab_patterns: Option<u16>,
    /// How many pad, bass and stab patterns there are to choose from (a choice among several is
    /// part of what a phrase sounds like, so it is in the phrase's fingerprint).
    pattern_counts: [u8; 3],
    comping: Comping,
    stab_hold_beats: f32,
    beats_per_bar: u32,
    call_and_response: bool,
    motif_scale: Option<Mode>,
    dynamics: Option<(f32, f32)>,
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
    /// Whose chord graph and whose motifs play: the dominant genre's own (its index) in a calm
    /// mood, else the mood's (`MOOD_SRC`).
    harm_src: u16,
    motif_src: u16,
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
    /// The piece this score plays (its seed's draws, its form and its place in it), the riffs it
    /// made and whose rules they came from (`u16::MAX`: none), its home key in the harmony playing,
    /// and the section the phrase now playing is (letter, time through the form, a return).
    piece: Piece,
    gen_motifs: [GenMotif; MAX_PIECE_MOTIFS],
    gen_n: usize,
    gen_src: u16,
    home_key: usize,
    section: Option<(u8, u64, bool)>,
    /// Tests only: every time through the form plays its sections exactly as the first (tune and
    /// chords), the looping piece the hour test's detector must catch.
    verbatim: bool,
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
            harm_src: MOOD_SRC,
            motif_src: MOOD_SRC,
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
            piece: Piece::new(seed),
            gen_motifs: [GenMotif::default(); MAX_PIECE_MOTIFS],
            gen_n: 0,
            gen_src: u16::MAX,
            home_key: 0,
            section: None,
            verbatim: false,
        }
    }

    /// Tests only: loop the form verbatim (see `verbatim`).
    #[cfg(test)]
    pub(crate) fn play_verbatim(&mut self) {
        self.verbatim = true;
    }

    /// Carry on the piece from its phrase `played` (a score rebuilt mid-piece, its data read again
    /// or a pack installed, takes over where the old one was: the same seed, the same form).
    pub fn resume_at(&mut self, played: u64) {
        self.piece.played = played;
    }

    /// The piece's seed.
    pub fn piece_seed(&self) -> u64 {
        self.piece.seed
    }

    /// The riffs this piece made (empty: its genre has no riff rules) and its form's letters (A = 0).
    pub fn piece_motifs(&self) -> &[GenMotif] {
        &self.gen_motifs[..self.gen_n]
    }

    pub fn piece_form(&self) -> &[u8] {
        &self.piece.form[..self.piece.form_len]
    }

    /// The instrument each role plays this phrase (None: the role is silent).
    pub fn role_instruments(&self) -> [Option<u16>; 8] {
        let mut out = [None; 8];
        if let Some(e) = &self.eff {
            for (k, r) in e.roles.iter().enumerate() {
                out[k] = r.map(|r| r.inst);
            }
        }
        out
    }

    /// The section playing: its letter + 1 (0: no form), which time through the form, and the
    /// piece's phrase index of it.
    pub fn section(&self) -> (u8, u32, u64) {
        match self.section {
            Some((l, c, _)) => (l + 1, c.min(u32::MAX as u64) as u32, self.piece.played.saturating_sub(1)),
            None => (0, 0, self.piece.played.saturating_sub(1)),
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

    /// Bars begun since the score started (a new genre takes over at the next bar line, BUG-297).
    pub fn bars(&self) -> u64 {
        self.bars_total
    }

    /// The recipe and level the genre gives the mix this phrase.
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

    /// Where in the bar the music is, in beats (2.4: the third beat, 40% through it), and how many
    /// beats a bar has: the Music page's beat pulse reads them from the score, not from the sound.
    pub fn beat_pos(&self) -> f32 {
        let phase = ((self.pos as f64 - self.beat_start) / self.beat_len.max(1.0)).clamp(0.0, 0.999);
        self.beat_in_bar as f32 + phase as f32
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.eff.as_ref().map(|e| e.beats_per_bar).unwrap_or(4)
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
            pad_patterns: None,
            bass_patterns: None,
            stab_patterns: None,
            pattern_counts: [0; 3],
            comping: Comping::default(),
            stab_hold_beats: 0.25,
            beats_per_bar: 4,
            call_and_response: false,
            motif_scale: None,
            dynamics: None,
            harmony: HarmonyRule::default(),
            recipe: Recipe { reverb_scale: 1.0, ..Default::default() },
            level_db: 0.0,
            tempo: (60.0, 60.0),
        })
    }

    fn steps_per_bar(&self) -> u32 {
        let e = self.eff();
        e.beats_per_bar.max(1) * e.subdivision.max(1)
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
        let eff = self.eff();
        let bpb = eff.beats_per_bar.max(1);
        let sub = eff.subdivision.max(1);
        let step_of_bar = self.beat_in_bar * sub + self.step_in_beat;
        let spb = self.steps_per_bar();
        let now = self.pos;
        let rate = self.rate;
        // Drums.
        if eff.beat_felt {
            for d in eff.drums.iter().flatten() {
                let Some(part) = drum_part(res, self.mood, d.genre, d.part) else { continue };
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
                    let note = NoteOn { inst: part.inst, role: 255, midi: part.midi as f32, a4: self.steer.a4, vel: (v * part.velocity * d.weight * (0.6 + 0.4 * self.plan.vel_scale)).min(1.0), pan: 0.0, hold_s: 0.0, tag, level: 1.0 };
                    if at > now {
                        self.schedule(at, note);
                    } else {
                        sink.note_on(note);
                    }
                }
            }
        }
        // The pad restruck on the genre's comp pattern (strummed low to high, and short, if the genre
        // says so: a gypsy-jazz guitar's chop).
        if let (Some(si), Some(pad)) = (eff.pad_patterns, eff.roles[RoleKind::Pad.index()]) {
            let pats = pad_pats(res, self.mood, si);
            if !pats.is_empty() {
                let pat = &pats[self.plan.patterns[5] as usize % pats.len()];
                if !pat.is_empty() {
                    let v = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    if v > 0.0 {
                        for k in 0..self.pads.n {
                            sink.note_off(self.pads.tags[k]);
                        }
                        self.pads.n = 0;
                        let strum = (eff.comping.strum_ms.max(0.0) as f64 * 0.001 * rate) as u64;
                        let hold_s = (eff.comping.hold_beats.max(0.0) as f64 * self.beat_len / rate) as f32;
                        for k in 0..self.pad_voicing.n {
                            let tag = self.next_tag();
                            let midi = self.pad_voicing.notes[k];
                            let vel = ((0.55 + 0.35 * v + self.rng.range(-0.05, 0.05)) * self.plan.vel_scale).clamp(0.0, 1.0);
                            let note = NoteOn { inst: pad.inst, role: RoleKind::Pad.index() as u8, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, hold_s, tag, level: pad.level };
                            if strum > 0 && k > 0 {
                                self.schedule(now + strum * k as u64, note);
                            } else {
                                sink.note_on(note);
                            }
                            self.pads.notes[k] = midi;
                            self.pads.tags[k] = tag;
                            self.pads.n = k + 1;
                        }
                    }
                }
            }
        }
        // A horn section's stabs: the choir role's voicing (the upper chord tones, voice-led) struck
        // short on the genre's pattern, on the chords the plan gives that role.
        if let (Some(si), Some(ch)) = (eff.stab_patterns, eff.roles[RoleKind::Choir.index()]) {
            let pats = stab_pats(res, si);
            if !pats.is_empty() && self.plan.choir[self.slot.min(MAX_SLOTS - 1)] && self.choir_voicing.n > 0 {
                let pat = &pats[self.plan.patterns[7] as usize % pats.len()];
                if !pat.is_empty() {
                    let v = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    if v > 0.0 {
                        let hold_s = (eff.stab_hold_beats as f64 * self.beat_len / rate) as f32;
                        for k in 0..self.choir_voicing.n {
                            let tag = self.next_tag();
                            let vel = ((0.6 + 0.4 * v) * self.plan.vel_scale * self.rng.range(0.9, 1.0)).min(1.0);
                            sink.note_on(NoteOn { inst: ch.inst, role: RoleKind::Choir.index() as u8, midi: self.choir_voicing.notes[k] as f32, a4: self.steer.a4, vel, pan: 0.0, hold_s, tag, level: ch.level });
                        }
                    }
                }
            }
        }
        // The bass on the genre's pattern.
        if let (Some(si), Some(bass)) = (eff.bass_patterns, eff.roles[RoleKind::Bass.index()]) {
            let pats = bass_pats(res, self.mood, si);
            if !pats.is_empty() {
                let pat = &pats[self.plan.patterns[6] as usize % pats.len()];
                if !pat.is_empty() {
                    let c = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    let root = self.chord_root_pc(res);
                    let prev = if self.bass.n > 0 { self.bass.notes[0] } else { (bass.low + bass.high) / 2 };
                    let walking = matches!(c, b'w' | b'W');
                    let off = match c {
                        b'x' | b'X' => Some(0),
                        b'5' => Some(7),
                        b'o' => Some(12),
                        b'3' => Some(if self.chord_has(root, 3) { 3 } else { 4 }),
                        b'w' | b'W' => Some(self.walking_step(res, root, prev, bass.low, bass.high, bpb)),
                        _ => None,
                    };
                    if let Some(off) = off {
                        for k in 0..self.bass.n {
                            sink.note_off(self.bass.tags[k]);
                        }
                        // A walking step is already a note in the register; the rest are intervals.
                        let target = if walking { off } else { nearest_pc_in(root + off, bass.low, bass.high, prev) };
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
        self.harmony(res).and_then(|h| h.chords.get(self.chord)).map(|c| (self.key_pc + c.root).rem_euclid(12)).unwrap_or(self.key_pc)
    }

    /// The key centres and chord graph playing: the genre's own progressions, or the mood's.
    fn harmony<'a>(&self, res: &'a Resolved) -> Option<&'a RHarmony> {
        if self.harm_src != MOOD_SRC {
            if let Some(h) = res.genres.get(self.harm_src as usize).and_then(|g| g.progressions.as_ref()) {
                return Some(h);
            }
        }
        res.moods.get(self.mood).map(|m| &m.harmony)
    }

    /// The motif bank playing: the genre's own, or the mood's.
    fn motifs<'a>(&self, res: &'a Resolved) -> &'a [Motif] {
        if self.motif_src != MOOD_SRC {
            if let Some(g) = res.genres.get(self.motif_src as usize).filter(|g| !g.motifs.is_empty()) {
                return &g.motifs;
            }
        }
        res.moods.get(self.mood).map(|m| m.motifs.as_slice()).unwrap_or(&[])
    }

    /// Whether the riffs playing are the ones this piece made.
    fn own_motifs(&self) -> bool {
        self.gen_n > 0 && self.motif_src == self.gen_src
    }

    /// How many riffs there are to choose from: the piece's own, else the bank's.
    fn motif_count(&self, res: &Resolved) -> usize {
        if self.own_motifs() {
            self.gen_n
        } else {
            self.motifs(res).len()
        }
    }

    /// Riff `i`: the piece's own, else the bank's (copied into the same fixed shape).
    fn motif_view(&self, res: &Resolved, i: usize) -> Option<GenMotif> {
        if self.own_motifs() {
            return (i < self.gen_n).then(|| self.gen_motifs[i]);
        }
        let m = self.motifs(res).get(i)?;
        let n = m.steps.len().min(m.beats.len()).min(crate::piece::MAX_MOTIF_NOTES);
        let mut g = GenMotif { n, ..Default::default() };
        for k in 0..n {
            g.steps[k] = m.steps[k];
            g.beats[k] = m.beats[k];
            g.hold[k] = m.hold.get(k).copied().unwrap_or(0.95);
        }
        Some(g)
    }

    /// The voices this piece chose for `g`'s lead roles (the call first, then the others, never two
    /// roles on one voice), each from the genre's leads for that role by weight and the piece's draw.
    fn lead_roles(&self, g: &crate::resolved::RGenre) -> [Option<EffRole>; 8] {
        let mut out: [Option<EffRole>; 8] = [None; 8];
        for (k, &i) in self.lead_picks(g).iter().enumerate() {
            if let Some((_, r, _)) = g.leads.get(i as usize) {
                out[k] = Some(EffRole { inst: r.inst, low: r.low, high: r.high, voices: r.voices, density: r.density, level: r.level });
            }
        }
        out
    }

    /// Which of `g`'s lead entries (indices in its `leads`) each role plays in this piece
    /// ([`crate::tune::PIECE`]: none): the Music page names the lead and the answering voice from it.
    pub fn lead_picks(&self, g: &crate::resolved::RGenre) -> [u16; 8] {
        let mut out = [crate::tune::PIECE; 8];
        if g.leads.is_empty() {
            return out;
        }
        let mut taken = [u16::MAX; 8];
        let mut order = [0usize; 8];
        order[0] = RoleKind::Melody.index();
        let mut n = 1;
        for k in 0..8 {
            if k != order[0] {
                order[n] = k;
                n += 1;
            }
        }
        for (t, &k) in order.iter().enumerate() {
            let w = |e: &(usize, RRole, f32)| if e.0 == k && !taken.contains(&e.1.inst) { e.2 } else { 0.0 };
            if !g.leads.iter().any(|e| w(e) > 0.0) {
                continue;
            }
            // The person's choice on the Music page (an entry of this genre's leads for this role),
            // else the piece's draw.
            let chosen = self.steer.tune.lead[k] as usize;
            let i = if chosen < g.leads.len() && g.leads[chosen].0 == k { chosen } else { crate::piece::pick(self.piece.lead_draw[k], g.leads.iter().map(w)) };
            out[k] = i as u16;
            taken[t] = g.leads[i].1.inst;
        }
        out
    }

    /// One step of a walking bass (a note in the register `low..=high`): the chord's root on its
    /// first beat (and usually on each bar's first beat); on the last beat before a new chord, a half
    /// step into that chord's root from the side the line is on (now and then its fifth, the
    /// dominant's way in); between, chord tones and scale steps heading for the next root.
    fn walking_step(&mut self, res: &Resolved, root: i32, prev: i32, low: i32, high: i32, bpb: u32) -> i32 {
        let slot = self.slot.min(MAX_SLOTS - 1);
        let last_bar = self.bars_into_chord >= self.plan.bars[slot] as u32;
        let next_root = if last_bar && self.slot + 1 < self.plan.n {
            self.harmony(res).and_then(|h| h.chords.get(self.plan.chords[self.slot + 1] as usize)).map(|c| (self.key_pc + c.root).rem_euclid(12)).unwrap_or(root)
        } else {
            root
        };
        if self.beat_in_bar == 0 && (self.bars_into_chord <= 1 || self.rng.chance(0.7)) {
            return nearest_pc_in(root, low, high, prev);
        }
        let goal = nearest_pc_in(next_root, low, high, prev);
        if last_bar && self.beat_in_bar + 1 == bpb {
            if self.rng.chance(0.25) {
                return nearest_pc_in(next_root + 7, low, high, prev);
            }
            let approach = if prev <= goal { goal - 1 } else { goal + 1 };
            return approach.clamp(low, high);
        }
        let mut dir = if goal > prev {
            1
        } else if goal < prev {
            -1
        } else if self.rng.chance(0.5) {
            1
        } else {
            -1
        };
        if prev + dir * 2 > high {
            dir = -1;
        } else if prev + dir * 2 < low {
            dir = 1;
        }
        // A chord tone that way within a fourth, else a step of the key's scale.
        if self.rng.chance(0.6) {
            for d in 1..=5 {
                let cand = prev + dir * d;
                if cand < low || cand > high {
                    break;
                }
                if self.chord_pcs[..self.chord_n].contains(&cand.rem_euclid(12)) {
                    return cand;
                }
            }
        }
        scale_step(prev, dir, self.key_pc, self.mode).clamp(low, high)
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
        let a = res.genres.get(self.steer.genre_a as usize);
        let b = if self.steer.blend > 0.001 { res.genres.get(self.steer.genre_b as usize) } else { None };
        let t = if b.is_some() { self.steer.blend.clamp(0.0, 1.0) } else { 0.0 };
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        let mut roles: [Option<EffRole>; 8] = [None; 8];
        let base = |r: &RRole| EffRole { inst: r.inst, low: r.low, high: r.high, voices: r.voices, density: r.density, level: r.level };
        // The piece's lead voices in each genre (in place of that genre's added role for the role).
        let leads_a = a.map(|s| self.lead_roles(s)).unwrap_or([None; 8]);
        let leads_b = b.map(|s| self.lead_roles(s)).unwrap_or([None; 8]);
        for k in 0..8 {
            let pick = |s: Option<&crate::resolved::RGenre>, leads: &[Option<EffRole>; 8]| -> Option<EffRole> {
                if let Some(l) = leads[k] {
                    return Some(l);
                }
                let off = s.is_some_and(|s| s.roles_off[k]);
                let mut r = match (&mood.roles[k], s.and_then(|s| s.roles_add[k].as_ref())) {
                    (_, Some(add)) => Some(base(add)),
                    (Some(m), None) if !off => Some(base(m)),
                    _ => None,
                }?;
                if let Some((inst, db, register)) = s.and_then(|s| s.palette[k]) {
                    r.inst = inst;
                    r.level *= crate::dsp::db_to_amp(db);
                    if let Some((lo, hi)) = register {
                        r.low = lo;
                        r.high = hi;
                    }
                }
                Some(r)
            };
            let ra = pick(a, &leads_a);
            let rb = if b.is_some() { pick(b, &leads_b) } else { None };
            roles[k] = if b.is_none() {
                ra
            } else {
                // Each role takes the second genre's instrument with a chance equal to the blend;
                // a role one genre lacks plays while the blend leans to the genre that has it.
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
        for (si, w) in [(self.steer.genre_a, 1.0 - t), (self.steer.genre_b, t)] {
            if w < 0.15 {
                continue;
            }
            if let Some(s) = res.genres.get(si as usize) {
                for p in 0..s.drums.len().min(8) {
                    if k < 8 {
                        drums[k] = Some(EffDrum { genre: si, part: p as u8, weight: w });
                        k += 1;
                    }
                }
            }
            if b.is_none() {
                break;
            }
        }
        let dominant = if t >= 0.5 { self.steer.genre_b } else { self.steer.genre_a };
        let dom = res.genres.get(dominant as usize);
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
            pad_patterns: dom.filter(|s| !s.pad_patterns.is_empty()).map(|_| dominant),
            bass_patterns: dom.filter(|s| !s.bass_patterns.is_empty()).map(|_| dominant),
            stab_patterns: dom.filter(|s| !s.stab_patterns.is_empty()).map(|_| dominant),
            pattern_counts: [0; 3],
            comping: dom.map(|s| s.comping).unwrap_or_default(),
            stab_hold_beats: dom.map(|s| s.stab_hold_beats).unwrap_or(0.25),
            beats_per_bar: dom.and_then(|s| s.beats_per_bar).unwrap_or(mood.beats_per_bar),
            call_and_response: dom.is_some_and(|s| s.call_and_response),
            motif_scale: dom.and_then(|s| s.motif_scale),
            dynamics: dom.and_then(|s| s.dynamics),
            harmony: dom.map(|s| s.harmony).unwrap_or_default(),
            recipe,
            level_db: lerp(a.map(|s| s.level_db).unwrap_or(0.0), b.map(|s| s.level_db).unwrap_or(0.0)),
            tempo: (lerp(tempo_a.0, tempo_b.0), lerp(tempo_a.1, tempo_b.1)),
        };
        // The mood's own drums and patterns: an urgent mood's always, a calm one's where the genre has
        // none (a mood that wants a gentle pulse under a genre that brings no rhythm).
        let genre_has_drums = eff.drums.iter().any(|d| d.is_some());
        if !mood.drums.is_empty() && (mood.urgent || !genre_has_drums) {
            eff.drums = [None; 8];
            for p in 0..mood.drums.len().min(8) {
                eff.drums[p] = Some(EffDrum { genre: MOOD_SRC, part: p as u8, weight: 1.0 });
            }
            if let Some(g) = mood.groove.filter(|_| !genre_has_drums || mood.urgent) {
                eff.subdivision = g.subdivision.clamp(1, 8);
                eff.swing = g.swing;
                eff.syncopation = g.syncopation;
                eff.humanize_ms = g.humanize_ms;
            }
        }
        if !mood.pad_patterns.is_empty() && (mood.urgent || eff.pad_patterns.is_none()) {
            eff.pad_patterns = Some(MOOD_SRC);
        }
        if !mood.bass_patterns.is_empty() && (mood.urgent || eff.bass_patterns.is_none()) {
            eff.bass_patterns = Some(MOOD_SRC);
        }
        if mood.urgent {
            // Danger plays as written in every genre: the mood's roles, tempo, beat and groove; the
            // genre keeps only its recipe (tape colour) and level.
            for k in 0..8 {
                eff.roles[k] = mood.roles[k].as_ref().filter(|m| self.steer.intensity >= m.min_intensity).map(base);
            }
            eff.tempo = mood.tempo;
            eff.beat_felt = mood.beat_felt;
            eff.harmony = HarmonyRule::default();
            eff.beats_per_bar = mood.beats_per_bar;
            eff.stab_patterns = None;
            eff.comping = Comping::default();
            eff.call_and_response = false;
            eff.motif_scale = None;
            eff.dynamics = None;
            let g = mood.groove.unwrap_or_default();
            eff.subdivision = g.subdivision.clamp(1, 8);
            eff.swing = g.swing;
            eff.syncopation = g.syncopation;
            eff.humanize_ms = g.humanize_ms;
        }
        let count = |n: usize| n.min(255) as u8;
        eff.pattern_counts = [
            count(eff.pad_patterns.map(|s| pad_pats(res, self.mood, s).len()).unwrap_or(0)),
            count(eff.bass_patterns.map(|s| bass_pats(res, self.mood, s).len()).unwrap_or(0)),
            count(eff.stab_patterns.map(|s| stab_pats(res, s).len()).unwrap_or(0)),
        ];
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
        // Whose chord graph and motifs play: the dominant genre's own in a calm mood, else the
        // mood's (a danger mood always plays its own, so danger sounds like danger in every genre).
        let dominant = if self.steer.blend >= 0.5 && res.genres.get(self.steer.genre_b as usize).is_some() { self.steer.genre_b } else { self.steer.genre_a };
        let calm = res.moods.get(target).is_some_and(|m| !m.urgent);
        let g = res.genres.get(dominant as usize).filter(|_| calm);
        let harm_src = if g.is_some_and(|g| g.progressions.is_some()) { dominant } else { MOOD_SRC };
        let motif_src = if g.is_some_and(|g| !g.motifs.is_empty() || g.motif_rules.is_some()) { dominant } else { MOOD_SRC };
        let harm_changed = first || harm_src != self.harm_src || (harm_src == MOOD_SRC && mood_changed);
        let prev_key = self.key_pc;
        self.mood = target;
        self.harm_src = harm_src;
        self.motif_src = motif_src;
        // The piece (crate::piece): its form comes from the dominant genre (a calm mood only; danger
        // plays free), its riffs from that genre's rules, both from the piece's seed.
        if let Some(gg) = g {
            if self.piece.form_genre != dominant {
                self.piece.choose_form(dominant, &gg.forms);
            }
            if let Some(rules) = gg.motif_rules.as_ref().filter(|_| self.gen_src != dominant) {
                let mut r = Rng::new(crate::piece::mix(self.piece.seed, 0x707 + dominant as u64));
                self.gen_n = crate::piece::make_motifs(rules, &mut r, &mut self.gen_motifs);
                self.gen_src = dominant;
            }
        }
        let section = if calm { self.piece.section(self.piece.played) } else { None };
        let Some(harm) = self.harmony(res) else { return };
        if harm_changed {
            // A piece's first phrase: the key its seed draws from the harmony's keys by weight. A
            // later change of harmony: the new harmony's key nearest the old one around the circle
            // of fifths, so a move between moods is a step, not a leap.
            self.key_idx = if first {
                crate::piece::pick(self.piece.key_draw, harm.keys.iter().map(|k| k.2))
            } else {
                (0..harm.keys.len()).min_by_key(|&i| fifths_distance(harm.keys[i].0, prev_key)).unwrap_or(0)
            };
            self.home_key = self.key_idx;
        } else if let Some((letter, _, _)) = section {
            // In a form: the A sections in the piece's home key; another section (the bridge) in a
            // second key when this piece's seed says its bridge moves (about a third of pieces).
            self.key_idx = if letter != 0 && harm.keys.len() > 1 && self.piece.bridge_moves < 0.35 {
                let home = self.home_key;
                crate::piece::pick(crate::piece::unit(crate::piece::mix(self.piece.seed, 0xb2 + letter as u64)), harm.keys.iter().enumerate().map(|(i, k)| if i == home { 0.0 } else { k.2 }))
            } else {
                self.home_key
            };
        } else if self.phrase_count % 4 == 0 {
            // A section's end: now and then the key moves to a related one (by the keys' weights).
            if harm.keys.len() > 1 && self.rng.chance(0.3) {
                let cur = self.key_idx;
                let pick = self.rng.weighted(harm.keys.iter().enumerate().map(|(i, k)| if i == cur { 0.0 } else { k.2 }));
                self.key_idx = pick;
            }
        }
        let m = &res.moods[self.mood];
        let (kpc, kmode, _) = harm.keys[self.key_idx.min(harm.keys.len() - 1)];
        // The person's transposition (the Music page's Key) moves every key the piece plays, its
        // bridge's included, so the piece keeps its shape.
        self.key_pc = (kpc + self.steer.tune.transpose.clamp(-6, 6) as i32).rem_euclid(12);
        self.mode = kmode;
        let mut eff = self.resolve_eff(res);
        if self.steer.tune.swing >= 0.0 {
            eff.swing = self.steer.tune.swing.clamp(0.5, 0.75);
        }
        self.eff = Some(eff);
        // Tempo: across the band for a mood that follows the intensity; in a form, the piece's own
        // place in the range; else chosen per phrase within the range.
        self.tempo = if res.moods[self.mood].tempo_follows_intensity {
            // Across the band: its slow end where the band starts, its fast end where the next begins.
            let froms = &res.intensity.froms;
            let lo = froms.get(self.band).copied().unwrap_or(0.0);
            let hi = froms.get(self.band + 1).copied().unwrap_or(1.0).max(lo + 1e-3);
            let x = ((self.steer.intensity - lo) / (hi - lo)).clamp(0.0, 1.0);
            eff.tempo.0 + (eff.tempo.1 - eff.tempo.0) * x
        } else if section.is_some() {
            self.steer.tune.tempo_for(eff.tempo.0 + (eff.tempo.1.max(eff.tempo.0) - eff.tempo.0) * self.piece.tempo_t, eff.tempo)
        } else {
            // The draw happens whatever the person chose, so a tempo change leaves the rest alone.
            let drawn = self.rng.range(eff.tempo.0, eff.tempo.1.max(eff.tempo.0));
            self.steer.tune.tempo_for(drawn, eff.tempo)
        }
        .clamp(20.0, 240.0);
        self.beat_len = self.rate * 60.0 / self.tempo as f64;
        let now_s = self.seconds() as f32;
        let window = m.no_repeat_s;
        let n_motifs = self.motif_count(res);
        let start_chord = if harm_changed { harm.start } else { self.chord };
        let plan = match section {
            Some((letter, chorus, _)) => {
                // A section of the piece: its chords from its own seed (the same letter is the same
                // chords all piece), its tune too the first time through the form; after that the
                // riffs are improvised over the same chords, re-drawn while they repeat a phrase
                // played within the window.
                let harmony_id = if harm_src == MOOD_SRC { 0x1000 + self.mood as u64 } else { harm_src as u64 };
                let mut sec_rng = Rng::new(self.piece.section_seed(harmony_id, letter));
                let choices = g.and_then(|g| g.phrase_bars.as_deref()).unwrap_or(&m.phrase_bars);
                let bars = choices[sec_rng.int(0, choices.len() as i32 - 1) as usize].clamp(1, 64);
                let chord_bars = eff.harmony.chord_bars.unwrap_or(m.chord_bars);
                let last_of_form = self.piece.form_len > 0 && (self.piece.played % self.piece.form_len as u64) as usize == self.piece.form_len - 1;
                let seed = self.piece.seed;
                let mut sec = section_chords(harm, &mut sec_rng, seed, letter, bars, chord_bars, last_of_form);
                if chorus > 0 && !self.verbatim {
                    // A later time through: the first half as before, the second half reharmonised
                    // (its own seed for this time through), so the section stays recognisable and
                    // the hour never plays the same two minutes twice.
                    let mut alt_rng = Rng::new(crate::piece::mix(self.piece.section_seed(harmony_id, letter), chorus));
                    let alt = section_chords(harm, &mut alt_rng, seed, letter, bars, chord_bars, last_of_form);
                    sec = crate::piece::splice(&sec, &alt, sec.total_bars() / 2);
                }
                let mut plan = self.make_plan(m, harm, n_motifs, start_chord, false, eff, Some((sec, (chorus == 0 || self.verbatim).then_some(sec_rng))));
                for _ in 0..24 {
                    let seen = chorus > 0 && !self.verbatim && self.history.iter().any(|&(h, t)| h == plan.hash && now_s - t < window);
                    if !seen {
                        break;
                    }
                    plan = self.make_plan(m, harm, n_motifs, start_chord, false, eff, Some((sec, None)));
                }
                plan
            }
            None => {
                // No form (a danger mood, a genre without one): every phrase walked afresh, re-made
                // while it repeats one played within the window.
                let mut plan = self.make_plan(m, harm, n_motifs, start_chord, !harm_changed, eff, None);
                for _ in 0..24 {
                    let seen = self.history.iter().any(|&(h, t)| h == plan.hash && now_s - t < window);
                    if !seen {
                        break;
                    }
                    plan = self.make_plan(m, harm, n_motifs, start_chord, !harm_changed, eff, None);
                }
                plan
            }
        };
        if calm {
            self.piece.played += 1;
        }
        self.section = section;
        self.history[self.hist_w] = (plan.hash, now_s);
        self.hist_w = (self.hist_w + 1) % HISTORY;
        self.plan = plan;
        self.bars_in_phrase = (0..plan.n).map(|i| plan.bars[i] as u32).sum::<u32>().max(1);
        self.bar_in_phrase = 0;
        self.slot = 0;
        self.bars_into_chord = 0;
        self.phrase_count += 1;
        let (sec_letter, chorus, returning) = section.map(|(l, c, r)| (l + 1, c.min(u32::MAX as u64) as u32, r && c == 0)).unwrap_or((0, 0, false));
        sink.event(Event::Phrase { hash: plan.hash, at_s: self.seconds(), mood: self.mood as u16, key_pc: self.key_pc, section: sec_letter, chorus, returning });
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
        // The texture runs under everything while the genre has one.
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

    /// A phrase's plan. `sec`: a section of the piece, its chords already made and, the first time
    /// through the form, the section's own generator for its tune (so a returning section plays the
    /// same tune); None: the chords walked afresh from `start`.
    fn make_plan(&mut self, m: &RMood, harm: &RHarmony, n_motifs: usize, start: usize, continuing: bool, eff: Eff, sec: Option<(Section, Option<Rng>)>) -> Plan {
        let mut p = Plan::default();
        let mut theme: Option<Rng> = None;
        if let Some((s, tune)) = sec {
            p.n = s.n.min(MAX_SLOTS);
            p.chords[..p.n].copy_from_slice(&s.chords[..p.n]);
            p.bars[..p.n].copy_from_slice(&s.bars[..p.n]);
            theme = tune;
        } else {
            let bars_total = m.phrase_bars[self.rng.int(0, m.phrase_bars.len() as i32 - 1) as usize].clamp(1, 64);
            let (lo, hi) = eff.harmony.chord_bars.unwrap_or(m.chord_bars);
            let (lo, hi) = (lo.max(1), hi.max(lo.max(1)));
            let mut cur = if continuing { self.walk(harm, start, false) } else { start };
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
                    cur = self.walk(harm, cur, last);
                }
            }
        }
        // Motifs (from the section's own generator for a tune, else improvised), the choir, the
        // pulse, the drum patterns.
        let dens = |r: RoleKind| eff.roles[r.index()].map(|x| x.density).unwrap_or(0.0);
        let (dc, dp) = (dens(RoleKind::Choir), dens(RoleKind::Percussion));
        let mut last_call: Option<MotifPlan> = None;
        for i in 0..p.n {
            p.motifs[i] = match theme.as_mut() {
                Some(t) => draw_motif(t, &eff, n_motifs, &mut last_call),
                None => draw_motif(&mut self.rng, &eff, n_motifs, &mut last_call),
            };
            p.choir[i] = self.rng.chance(dc);
        }
        p.pulse = self.rng.chance(dp);
        for k in 0..8 {
            p.patterns[k] = self.rng.int(0, 7) as u8;
        }
        p.vel_scale = match eff.dynamics {
            Some((lo, hi)) => self.rng.range(lo, hi.max(lo)),
            None => 1.0,
        };
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
        for (k, &n) in eff.pattern_counts.iter().enumerate() {
            if n > 1 {
                h = fnv(h, (p.patterns[5 + k] as usize % n as usize) as u64 | (k as u64) << 8);
            }
        }
        if eff.dynamics.is_some() {
            h = fnv(h, (p.vel_scale * 8.0) as u64);
        }
        p.hash = h;
        p
    }

    /// One step along the chord graph from `cur`; `cadence` asks for a phrase-ending chord.
    /// (Each move's weight leans by this piece's seed, crate::piece::lean.)
    fn walk(&mut self, h: &RHarmony, cur: usize, cadence: bool) -> usize {
        crate::piece::walk(&h.moves, &h.cadence, &mut self.rng, self.piece.seed, cur, cadence)
    }

    fn change_chord(&mut self, res: &Resolved, sink: &mut dyn Sink, chord: usize) {
        let Some(harm) = self.harmony(res) else { return };
        let Some(c) = harm.chords.get(chord) else { return };
        self.chord = chord;
        let eff = self.eff();
        // The chord's pitch classes under the genre's harmony rules.
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
        let beats = self.plan.bars[self.slot] as f32 * eff.beats_per_bar as f32;
        let mask = pcs[..n].iter().fold(0u16, |acc, &p| acc | 1 << p);
        sink.event(Event::Chord { chord: chord as u16, root: root_pc as i8, pcs: mask, at_s: self.seconds(), beats });
        let at_s = self.seconds();
        // Pads: voice-led.
        if let Some(pad) = eff.roles[RoleKind::Pad.index()] {
            let prev = self.pad_voicing;
            let notes = voice_lead(&prev.notes[..prev.n], pad.voices as usize, &pcs[..n], root_pc, pad.low, pad.high);
            self.pad_voicing = Held { notes, tags: [0; 8], n: pad.voices as usize };
            sink.event(Event::Voicing { role: RoleKind::Pad.index() as u8, notes, n: pad.voices, at_s });
            if eff.pad_patterns.is_none() {
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
            Some(ch) if choir_on && eff.stab_patterns.is_some() => {
                // Stabs: the voicing is led and remembered here; the genre's pattern strikes it short
                // (on_step), so nothing is held.
                let led = self.choir_voicing;
                let notes = voice_lead(&led.notes[..led.n], ch.voices as usize, &pcs[..n], root_pc, ch.low, ch.high);
                self.choir_voicing = Held { notes, tags: [0; 8], n: ch.voices as usize };
                sink.event(Event::Voicing { role: RoleKind::Choir.index() as u8, notes, n: ch.voices, at_s });
                for k in 0..self.choir.n {
                    sink.note_off(self.choir.tags[k]);
                }
                self.choir.n = 0;
            }
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
        // Bass, held (a genre's pattern plays it on its steps instead).
        if let (Some(b), None) = (eff.roles[RoleKind::Bass.index()], eff.bass_patterns) {
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
            if let (Some(r), Some(motif)) = (eff.roles[role_idx], self.motif_view(res, mp.motif as usize).filter(|m| m.n > 0)) {
                // Anchor: one of the chord's tones placed in the role's register.
                let tone = pcs[(mp.anchor as usize) % n.max(1)];
                let anchor = nearest_pc_in(tone, r.low, (r.high - 7).max(r.low), (r.low + r.high) / 2);
                let scale = eff.motif_scale.unwrap_or(self.mode.pentatonic());
                let chord_beats = beats;
                let mut beat = mp.start_halfbeats as f32 * 0.5;
                let first = clamp_in_scale(scale_step(anchor, motif.steps[0], self.key_pc, scale), r.low, r.high, self.key_pc, scale);
                sink.event(Event::Motif { role: role_idx as u8, first_note: first, at_s });
                for (k, (&st, &bt)) in motif.steps[..motif.n].iter().zip(motif.beats[..motif.n].iter()).enumerate() {
                    if beat >= chord_beats {
                        break;
                    }
                    let step = st + if k == motif.n - 1 { mp.vary as i32 } else { 0 };
                    let midi = clamp_in_scale(scale_step(anchor, step, self.key_pc, scale), r.low, r.high, self.key_pc, scale);
                    let at = self.pos + self.beats_to_samples(beat, eff) as u64;
                    let tag = self.next_tag();
                    let vel = (mp.vel * self.rng.range(0.85, 1.0) * self.plan.vel_scale).min(1.0);
                    let len = bt * if mp.loosen && k == 1 { 1.5 } else { 1.0 };
                    let pan = self.rng.range(-0.45, 0.45);
                    self.schedule(at, NoteOn { inst: r.inst, role: role_idx as u8, midi: midi as f32, a4: self.steer.a4, vel, pan, hold_s: (len * self.beat_len as f32 / self.rate as f32) * motif.hold[k], tag, level: r.level });
                    beat += len;
                }
            }
        }
    }

    /// A position in beats from now as samples, with the genre's swing on the off-steps.
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

/// A drum part's or a pattern's source meaning "the mood's own", not a genre's.
const MOOD_SRC: u16 = u16::MAX;

/// A section's chords in `harm`: from its grammar when it has one, else walked from its chord graph
/// (section A from the graph's start, another letter from a step away), `bars` bars long.
fn section_chords(harm: &RHarmony, rng: &mut Rng, seed: u64, letter: u8, bars: u32, chord_bars: (u32, u32), end_home: bool) -> Section {
    match harm.grammar.as_ref() {
        Some(gr) => crate::piece::grammar_section(harm.chords.len(), gr, rng, seed, bars, chord_bars, end_home),
        None => {
            let start = if letter == 0 { harm.start } else { crate::piece::walk(&harm.moves, &harm.cadence, rng, seed, harm.start, false) };
            crate::piece::graph_section(&harm.moves, &harm.cadence, rng, seed, start, bars, chord_bars)
        }
    }
}

/// A scale note brought inside `lo..=hi` by scale steps (a riff's note past the role's register
/// moves back along the scale; a plain clamp could land between scale notes: with D6 the top of the
/// register, a riff in C blues would have played a D, outside the scale).
fn clamp_in_scale(m: i32, lo: i32, hi: i32, key_pc: i32, mode: Mode) -> i32 {
    let mut m = m;
    let mut guard = 0;
    while m > hi && guard < 24 {
        m = scale_step(m, -1, key_pc, mode);
        guard += 1;
    }
    while m < lo && guard < 48 {
        m = scale_step(m, 1, key_pc, mode);
        guard += 1;
    }
    if m > hi || m < lo {
        m.clamp(lo, hi)
    } else {
        m
    }
}

/// One chord's motif in a plan, drawn with `rng` from `n_motifs` riffs. Call and response: the
/// melody role calls on one chord (`last_call` remembers it); on the next the bells role answers
/// with the same riff and rhythm from another chord tone, ending a step lower. Otherwise the bells
/// or the melody, by their densities, now and then plays a riff.
fn draw_motif(rng: &mut Rng, eff: &Eff, n_motifs: usize, last_call: &mut Option<MotifPlan>) -> MotifPlan {
    let dens = |r: RoleKind| eff.roles[r.index()].map(|x| x.density).unwrap_or(0.0);
    let (db, dm) = (dens(RoleKind::Bells), dens(RoleKind::Melody));
    let mut mp = MotifPlan::default();
    if n_motifs == 0 {
        return mp;
    }
    if eff.call_and_response {
        if let Some(call) = last_call.take().filter(|_| eff.roles[RoleKind::Bells.index()].is_some()) {
            mp = call;
            mp.role = RoleKind::Bells.index() as u8 + 1;
            mp.anchor = (call.anchor + 1) % 4;
            mp.vary = -1;
            mp.vel = call.vel * 0.95;
        } else if eff.roles[RoleKind::Melody.index()].is_some() && rng.chance(dm) {
            mp.role = RoleKind::Melody.index() as u8 + 1;
            mp.motif = rng.int(0, n_motifs as i32 - 1) as u8;
            mp.start_halfbeats = rng.int(0, 2) as u8;
            mp.anchor = rng.int(0, 3) as u8;
            mp.vary = rng.int(0, 1) as i8;
            mp.loosen = rng.chance(0.2);
            mp.vel = rng.range(0.7, 1.0);
            *last_call = Some(mp);
        }
    } else {
        let role = if eff.roles[RoleKind::Bells.index()].is_some() && rng.chance(db) {
            Some(RoleKind::Bells)
        } else if eff.roles[RoleKind::Melody.index()].is_some() && rng.chance(dm) {
            Some(RoleKind::Melody)
        } else {
            None
        };
        if let Some(r) = role {
            mp.role = r.index() as u8 + 1;
            mp.motif = rng.int(0, n_motifs as i32 - 1) as u8;
            mp.start_halfbeats = rng.int(1, 6) as u8;
            mp.anchor = rng.int(0, 3) as u8;
            mp.vary = rng.int(-1, 1) as i8;
            mp.loosen = rng.chance(0.3);
            mp.vel = rng.range(0.6, 1.0);
        }
    }
    mp
}

/// A drum part from the mood (`MOOD_SRC`) or a genre.
fn drum_part(res: &Resolved, mood: usize, genre: u16, part: u8) -> Option<&crate::resolved::RDrum> {
    if genre == MOOD_SRC {
        res.moods.get(mood)?.drums.get(part as usize)
    } else {
        res.genres.get(genre as usize)?.drums.get(part as usize)
    }
}

/// The pad patterns of the mood (`MOOD_SRC`) or a genre.
fn pad_pats(res: &Resolved, mood: usize, src: u16) -> &[Vec<f32>] {
    if src == MOOD_SRC {
        res.moods.get(mood).map(|m| m.pad_patterns.as_slice()).unwrap_or(&[])
    } else {
        res.genres.get(src as usize).map(|s| s.pad_patterns.as_slice()).unwrap_or(&[])
    }
}

/// The bass patterns of the mood (`MOOD_SRC`) or a genre.
fn bass_pats(res: &Resolved, mood: usize, src: u16) -> &[Vec<u8>] {
    if src == MOOD_SRC {
        res.moods.get(mood).map(|m| m.bass_patterns.as_slice()).unwrap_or(&[])
    } else {
        res.genres.get(src as usize).map(|s| s.bass_patterns.as_slice()).unwrap_or(&[])
    }
}

/// A genre's stab patterns (a mood has none).
fn stab_pats(res: &Resolved, src: u16) -> &[Vec<f32>] {
    res.genres.get(src as usize).map(|s| s.stab_patterns.as_slice()).unwrap_or(&[])
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
