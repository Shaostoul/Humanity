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
//!
//! SEEKING (music.md section 7h, 2026-11-21). A calm piece is PLANNED whole before its notes
//! (crate::arc: every phrase's section, bars, stage and energy), and everything the composer draws
//! comes from seeds derived from the piece's seed, never from a stream carried along from the start:
//! the plan from [`plan_seed`], each phrase's improvisation from [`phrase_seed`], each bar's own
//! choices (drum timing, velocities, the walking bass, the heartbeat, the riffs' notes) from
//! [`bar_seed`], each section's chords from its section seed. Voicings are placed against the role's
//! register, never led from a voicing long past, and a phrase's first bass note is placed from the
//! middle of the register, so what a phrase plays depends only on the phrase before it.
//! [`Composer::seek_to_bar`] therefore reaches bar N by planning the piece and playing, silently, from
//! the start of the phrase before the one bar N is in: the same work at minute 1 and on day 3, and bar
//! N then equals bar N reached by playing from the start, note for note. What the seed does not
//! decide: the world's and the person's changes on top (danger and action, a move to another mood, a
//! blend, the Music page's tune), which steer the composer as they happen.

use crate::arc::{ArcPlan, PlanPhrase, FREE, ROUND_CHORUS};
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
    /// `stage`: the planned stage it is in (an index in the genre's arc; 255: not planned, danger),
    /// and its planned energy (1 = as levelled).
    Phrase { hash: u64, at_s: f64, mood: u16, key_pc: i32, section: u8, chorus: u32, returning: bool, stage: u8, energy: f32 },
    /// A chord began: which, its root's pitch class, its pitch classes as a 12-bit mask, how many
    /// beats it lasts (the Music page names it from the root and the mask: "Dm6", "A7").
    Chord { chord: u16, root: i8, pcs: u16, at_s: f64, beats: f32 },
    /// A role's voicing changed (pads and choir): its notes, lowest first.
    Voicing { role: u8, notes: [i32; 8], n: u8, at_s: f64 },
    /// A motif began on a role.
    Motif { role: u8, first_note: i32, at_s: f64 },
    /// The intensity band changed at a bar line (0 = calm, the place's own mood).
    Band { band: u16, at_s: f64 },
    /// A section of the genre's structure began (rung E1): its index in the structure, whether it
    /// is a build or a drop, and how many bars it lasts.
    Structure { index: u8, build: bool, drop: bool, bars: u32, at_s: f64 },
}

/// Where the composer's decisions go.
pub trait Sink {
    fn note_on(&mut self, note: NoteOn);
    fn note_off(&mut self, tag: u32);
    fn event(&mut self, _e: Event) {}
    /// A drum that ducks has hit: the roles `roles` (one bit each) dip by `depth_db`, reaching it
    /// in `attack_s` and coming back over `release_s` (a genre's sidechain).
    fn duck(&mut self, _roles: u8, _depth_db: f32, _attack_s: f32, _release_s: f32) {}
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
    /// The piece's length, seconds: its long arc is planned over it (0: no length, the genre's form
    /// over and over; the game always sets it, from "A new piece").
    pub piece_s: f32,
}

/// The seed scheme (music.md section 7h). Every seed is `piece::mix(parent, salt or number)` with the
/// piece's seed at the root: the plan's, each phrase's, each bar's.
const SALT_PLAN: u64 = 0xa2c0;
const SALT_PHRASE: u64 = 0x9a5e;
const SALT_BAR: u64 = 0xba70;
/// A bar outside the plan (a danger mood's): keyed by the bars since the score began.
const SALT_FREE_BAR: u64 = 0xda90;
/// How many times an improvised phrase may be drawn again while it sounds like a phrase of its letter
/// in the mood's no-repeat window.
const ATTEMPTS: u64 = 6;

/// The seed a piece's plan is laid out from, for the genre keyed `genre_key`.
pub fn plan_seed(piece: u64, genre_key: u64) -> u64 {
    crate::piece::mix(piece, SALT_PLAN ^ genre_key)
}

/// The seed planned phrase `i` (counted over every round of the arc) improvises from.
pub fn phrase_seed(piece: u64, i: u64) -> u64 {
    crate::piece::mix(crate::piece::mix(piece, SALT_PHRASE), i)
}

/// The seed piece bar `bar` draws its own choices from.
pub fn bar_seed(piece: u64, bar: u64) -> u64 {
    crate::piece::mix(crate::piece::mix(piece, SALT_BAR), bar)
}

/// A sink that keeps nothing (a seek's silent run up to its bar).
pub struct Silent;

impl Sink for Silent {
    fn note_on(&mut self, _note: NoteOn) {}
    fn note_off(&mut self, _tag: u32) {}
}

/// The register's reference voicing for `n` voices in `lo..=hi`: evenly spaced notes, clear of both ends
/// (the lowest a step up from the register's floor, so a pad's low voice does not rumble). Every chord is
/// voiced nearest it (crate::composer::voice_lead from it), so a voicing depends on its chord alone and
/// two chords in a row never move a voice far (each sits near the same reference).
pub fn register_ref(n: usize, lo: i32, hi: i32) -> [i32; 8] {
    let n = n.clamp(1, 8);
    let span = (hi - lo).max(1) as f32;
    let mut out = [0i32; 8];
    for (k, o) in out.iter_mut().take(n).enumerate() {
        *o = (lo as f32 + span * (k as f32 + 1.0) / (n as f32 + 1.0)).round() as i32;
    }
    out
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
    /// The dominant genre's sidechain in a calm mood (roles, depth dB, attack, release).
    sidechain: Option<(u8, f32, f32, f32)>,
    /// The dominant genre's tempo delay and gated reverb in a calm mood (rung E3).
    delay: Option<crate::fx::RDelay>,
    gated: Option<crate::fx::RGated>,
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
    /// Tags: the bar's own range (its piece bar, or bars since the start with the top bit for a bar
    /// outside the plan, shifted 12 bits) and the count within it, so a bar's tags are the same
    /// however it was reached.
    tag_base: u32,
    tag_k: u32,
    started: bool,
    /// The long arc (crate::arc): the piece laid out, what it was laid out from, and the planned phrase
    /// playing with the piece bar it began on (None: a danger mood's phrase, outside the plan).
    arc: ArcPlan,
    arc_key: u64,
    planned: Option<(PlanPhrase, u64)>,
    /// The piece bar the last planned phrase began on (the status's place in the piece; a rebuild
    /// carries on from it).
    form_bar: u64,
    /// The phrase's first bass note is placed from the middle of the register (so a phrase never
    /// depends on the bass line of the one before).
    bass_fresh: bool,
    /// The home key moved with a change of harmony mid-piece (the world's doing, not the seed's).
    home_adapted: bool,
    /// The beat the tempo is counted from (a sample) and the beats since: each beat's start is
    /// computed, never summed, so it lands on the same sample however the bar was reached.
    tempo_origin: f64,
    tempo_beats: u64,
    /// Strike what is held again at the next decision (a seek: the harmony sounds from its bar).
    restrike: bool,
    /// The pad's and the choir's registers as last voiced (voices, low, high; 0 voices: none yet),
    /// and whether each is still moving from an old register into a new one (crate::composer::voicing).
    regs: [(u8, i32, i32); 2],
    unsettled: [bool; 2],
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
    /// The structure's section playing (its index; None: the genre has no structure, or danger),
    /// and a build's silence: the sample it begins at (`u64::MAX`: none), and whether the music has
    /// let go for it.
    part: Option<(u16, u8)>,
    /// Beats begun since the score began (the beat clock's whole beats).
    beats_total: u64,
    gap_from: u64,
    gap_done: bool,
    /// A section's tape stop (rung E3): the samples it runs from and to (`u64::MAX`: none). Inside
    /// it nothing new is struck and nothing held lets go; the score reads the speed.
    tape_from: u64,
    tape_to: u64,
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
            tag_base: 0,
            tag_k: 0,
            started: false,
            arc: ArcPlan::default(),
            arc_key: 0,
            planned: None,
            form_bar: 0,
            bass_fresh: true,
            home_adapted: false,
            tempo_origin: 0.0,
            tempo_beats: 0,
            restrike: false,
            regs: [(0, 0, 0); 2],
            unsettled: [false; 2],
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
            part: None,
            beats_total: 0,
            gap_from: u64::MAX,
            gap_done: false,
            tape_from: u64::MAX,
            tape_to: u64::MAX,
        }
    }

    /// The tape's speed at sample `at` (rung E3): 1 outside a tape stop; inside one, falling in a
    /// straight line from 1 at its start to 0 at its end (a motor running down at a steady rate).
    pub fn tape_speed_at(&self, at: u64) -> f32 {
        if at < self.tape_from || at >= self.tape_to {
            return 1.0;
        }
        1.0 - ((at - self.tape_from) as f64 / (self.tape_to - self.tape_from).max(1) as f64) as f32
    }

    /// The sample now (the composer's clock).
    pub fn pos(&self) -> u64 {
        self.pos
    }

    /// Whether a tape stop is running now.
    pub fn in_tape_stop(&self) -> bool {
        self.pos >= self.tape_from && self.pos < self.tape_to
    }

    /// The tempo delay and the gated reverb playing (the dominant genre's, in a calm mood).
    pub fn fx(&self) -> (Option<crate::fx::RDelay>, Option<crate::fx::RGated>) {
        self.eff.as_ref().map(|e| (e.delay, e.gated)).unwrap_or((None, None))
    }

    /// The structure's section playing (an index into the genre's structure), if any.
    pub fn structure_section(&self) -> Option<u8> {
        self.part.map(|(_, i)| i)
    }

    /// The structure playing (the genre it is from), in a calm mood.
    fn structure<'a>(&self, res: &'a Resolved) -> Option<&'a crate::resolved::RStructure> {
        res.genres.get(self.part?.0 as usize)?.structure.as_ref()
    }

    /// The structure's section playing.
    fn part_section(&self, res: &Resolved) -> Option<crate::resolved::RSection> {
        self.structure(res)?.sections.get(self.part?.1 as usize).copied()
    }

    /// Where the music is in time, for LFOs in time with it: the bar now playing (counted from the
    /// score's start) and the beats since the score began.
    pub fn beat_clock(&self) -> (u64, f64) {
        let phase = ((self.pos as f64 - self.beat_start) / self.beat_len.max(1.0)).clamp(0.0, 0.999_999);
        (self.bars_total.saturating_sub(1), self.beats_total as f64 + phase)
    }

    /// Tests only: loop the form verbatim (see `verbatim`).
    #[cfg(test)]
    pub(crate) fn play_verbatim(&mut self) {
        self.verbatim = true;
    }

    /// The piece laid out (crate::arc): its phrases, their stages and energies (empty before the first
    /// calm phrase, or outside the plan).
    pub fn plan(&self) -> &ArcPlan {
        &self.arc
    }

    /// The planned phrase playing and the piece bar it began on (None: outside the plan).
    pub fn planned(&self) -> Option<(PlanPhrase, u64)> {
        self.planned
    }

    /// The piece bar now playing (the plan's clock; the bar of the last planned phrase while a danger
    /// mood plays).
    pub fn piece_bar(&self) -> u64 {
        match self.planned {
            Some((_, start)) => start + self.bar_in_phrase as u64,
            None => self.form_bar,
        }
    }

    /// Strike what the composer holds again at its next decision (after [`Composer::seek_to_bar`],
    /// so the chord and the drone sound from the bar sought, not only from the next change).
    pub fn strike_held_next(&mut self) {
        self.restrike = true;
    }

    /// SEEK: make the composer stand at the start of piece bar `bar`, as if it had played the piece from
    /// its start, without playing it (music.md section 7h). The piece is planned for the steer as it
    /// is, then the composer runs silently (into `sink`: [`Silent`] to drop it, a recorder to keep it)
    /// from the start of the phrase before the one `bar` is in, up to `bar`'s first decision; the next
    /// [`Composer::process`] plays `bar` exactly as playing from the start would have. The cost is at
    /// most two phrases of composing, whatever the bar. A steer outside the plan (no genre, a danger
    /// mood) starts afresh at the bar's time with nothing planned before it.
    pub fn seek_to_bar(&mut self, res: &Resolved, bar: u64, sink: &mut dyn Sink) {
        let target = (self.steer.mood as usize).min(res.moods.len().saturating_sub(1));
        let Some(mood) = res.moods.get(target) else { return };
        let dominant = if self.steer.blend >= 0.5 && res.genres.get(self.steer.genre_b as usize).is_some() { self.steer.genre_b } else { self.steer.genre_a };
        let g = res.genres.get(dominant as usize).filter(|_| !mood.urgent);
        // A fresh composer at the phrase before the bar's.
        self.mood = target;
        self.harm_src = MOOD_SRC;
        self.motif_src = MOOD_SRC;
        let mut from = 0;
        let mut from_bar = bar;
        if let Some(gg) = g {
            match &gg.structure {
                Some(st) => self.piece.choose_form(dominant, gg.seed_key, &st.form),
                None => self.piece.choose_form(dominant, gg.seed_key, &gg.forms),
            }
            self.ensure_plan(res, dominant, gg);
            if let Some(i) = self.arc.phrase_at_bar(bar) {
                from = i.saturating_sub(1);
                from_bar = self.arc.phrase(from).map(|p| p.2).unwrap_or(0);
            }
        }
        for h in [&mut self.pads, &mut self.pad_voicing, &mut self.choir, &mut self.choir_voicing, &mut self.bass, &mut self.drone, &mut self.texture] {
            h.n = 0;
        }
        self.drone_root = -1;
        self.pending = [Pending::default(); PENDING];
        self.history = [(0, f32::NEG_INFINITY); HISTORY];
        self.hist_w = 0;
        self.phrase_count = 0;
        self.band = 0;
        self.pending_band = None;
        self.gap_from = u64::MAX;
        self.gap_done = false;
        self.tape_from = u64::MAX;
        self.tape_to = u64::MAX;
        self.planned = None;
        self.section = None;
        self.part = None;
        self.home_adapted = false;
        self.restrike = false;
        self.regs = [(0, 0, 0); 2];
        self.unsettled = [false; 2];
        self.started = true;
        // The piece's tempo and bar from the genre and the mood the seek plays in.
        let e = self.resolve_eff(res, &mut Rng::new(0));
        let bpb = e.beats_per_bar.max(1) as u64;
        self.tempo = self.piece_tempo(e.tempo);
        self.beat_len = self.rate * 60.0 / self.tempo as f64;
        self.eff = Some(e);
        self.tempo_origin = 0.0;
        self.tempo_beats = from_bar * bpb;
        self.beat_start = self.tempo_origin + self.tempo_beats as f64 * self.beat_len;
        self.next_step = self.beat_start;
        self.pos = self.beat_start.ceil() as u64;
        self.step_in_beat = 0;
        self.beat_in_bar = 0;
        self.bar_in_phrase = 0;
        self.bars_in_phrase = 0;
        self.bars_total = from_bar;
        self.beats_total = from_bar * bpb;
        self.piece.played = from;
        self.form_bar = from_bar;
        // Play silently up to the bar's first step (its start is computed as the run will compute it).
        let stop = (self.tempo_origin + (bar * bpb) as f64 * self.beat_len).ceil() as u64;
        let mut guard = 0u64;
        while self.pos < stop && guard < 4_000_000 {
            guard += 1;
            self.process(res, sink);
            let n = self.until_next().min((stop - self.pos) as usize).max(1);
            self.advance(n);
        }
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

    /// The section playing: its letter + 1 (0: no form), which time through the form, and the piece
    /// bar its phrase began on (a rebuild carries on from there: [`Composer::seek_to_bar`]).
    pub fn section(&self) -> (u8, u32, u64) {
        match self.section {
            Some((l, c, _)) if l != FREE => (l + 1, c.min(u32::MAX as u64) as u32, self.form_bar),
            _ => (0, 0, self.form_bar),
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
        self.tempo_origin *= k;
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
            self.tempo_origin = self.beat_start;
            self.tempo_beats = 0;
        }
        if self.restrike {
            self.restrike = false;
            self.strike_held(sink);
        }
        let in_gap = self.pos >= self.gap_from || self.in_tape_stop();
        for i in 0..PENDING {
            if let Some(n) = self.pending[i].note {
                if self.pending[i].at <= self.pos {
                    self.pending[i].note = None;
                    // A build's silence (or a tape stop): a riff note due in it is not played.
                    if in_gap && (n.role as usize) < 8 {
                        continue;
                    }
                    sink.note_on(n);
                }
            }
        }
        // A step is due once its time has come, on the first sample at or after it, wherever the
        // caller cuts its blocks (so a seek's run and the score's agree to the sample).
        let mut guard = 0;
        while self.next_step <= self.pos as f64 && guard < 64 {
            guard += 1;
            self.on_step(res, sink);
        }
    }

    /// A role's voicing of a chord (`slot` 0 the pad, 1 the choir). Settled (always, in a steady piece):
    /// each voice a chord tone within its WINDOW of the register's reference ([`register_ref`], from
    /// two semitones under to three over), so a voicing depends on its chord alone and two in a row
    /// never move a voice more than five semitones (a fourth). After a move to a mood whose register
    /// differs (the world's doing): led from the last voicing (each voice at most a fourth) and pulled
    /// toward the new reference, until a voicing lands inside the windows.
    #[allow(clippy::too_many_arguments)]
    fn voicing(&mut self, slot: usize, n: usize, lo: i32, hi: i32, prev: &[i32], pcs: &[i32], root_pc: i32) -> [i32; 8] {
        let n = n.clamp(1, 8);
        let reg = (n as u8, lo, hi);
        if self.regs[slot] != reg {
            self.unsettled[slot] = self.regs[slot].0 != 0;
            self.regs[slot] = reg;
        }
        let r = register_ref(n, lo, hi);
        if self.unsettled[slot] && prev.len() == n {
            let v = voice_lead_within(prev, n, pcs, root_pc, lo, hi, (-5, 5), Some(&r));
            if (0..n).all(|k| v[k] >= r[k] - 2 && v[k] <= r[k] + 3) {
                self.unsettled[slot] = false;
            }
            return v;
        }
        self.unsettled[slot] = false;
        voice_lead_within(&r[..n], n, pcs, root_pc, lo, hi, (-2, 3), None)
    }

    /// Strike again, with their own tags, the notes the composer holds (after a seek).
    fn strike_held(&mut self, sink: &mut dyn Sink) {
        let eff = self.eff();
        let a4 = self.steer.a4;
        for (h, role) in [(self.pads, RoleKind::Pad), (self.choir, RoleKind::Choir), (self.bass, RoleKind::Bass), (self.drone, RoleKind::Drone), (self.texture, RoleKind::Texture)] {
            let Some(r) = eff.roles[role.index()] else { continue };
            for k in 0..h.n {
                sink.note_on(NoteOn { inst: r.inst, role: role.index() as u8, midi: h.notes[k] as f32, a4, vel: 0.8, pan: 0.0, gate: 0.0, hold_s: 0.0, tag: h.tags[k], level: r.level });
            }
        }
    }

    /// The tempo a planned piece keeps, from the genre's (or the mood's) range `range`: the piece's own
    /// place in it, or the person's tempo on the Music page.
    fn piece_tempo(&self, range: (f32, f32)) -> f32 {
        self.steer.tune.tempo_for(range.0 + (range.1.max(range.0) - range.0) * self.piece.tempo_t, range).clamp(20.0, 240.0)
    }

    /// Lay the piece out for genre `dominant` (`g`) in the mood playing, unless the plan already in hand
    /// was laid out from the same things (the genre, the length in bars, the section lengths, the seed).
    fn ensure_plan(&mut self, res: &Resolved, dominant: u16, g: &crate::resolved::RGenre) {
        let Some(m) = res.moods.get(self.mood) else { return };
        let e = self.resolve_eff(res, &mut Rng::new(0));
        let tempo = self.piece_tempo(e.tempo);
        let bar_s = e.beats_per_bar.max(1) as f64 * 60.0 / tempo.max(1.0) as f64;
        let arc = g.arc.as_ref().filter(|_| self.steer.piece_s > 0.0);
        let length_bars = if arc.is_some() { (self.steer.piece_s as f64 / bar_s).round().clamp(1.0, 1e6) as u32 } else { 0 };
        let phrase_bars: &[u32] = g.phrase_bars.as_deref().unwrap_or(&m.phrase_bars);
        let mut key = fnv(fnv(fnv(0xcbf2_9ce4_8422_2325, dominant as u64), length_bars as u64), self.piece.seed);
        for &b in phrase_bars {
            key = fnv(key, b as u64);
        }
        for &l in &self.piece.form[..self.piece.form_len] {
            key = fnv(key, l as u64 | 0x100);
        }
        if key == self.arc_key && self.arc.n > 0 {
            return;
        }
        let mut structure_bars = [0u32; crate::piece::MAX_FORM];
        let mut sn = 0;
        if let Some(st) = &g.structure {
            for (k, s) in st.sections.iter().take(crate::piece::MAX_FORM).enumerate() {
                structure_bars[k] = s.bars;
                sn = k + 1;
            }
        }
        let form = self.piece.form;
        let input = crate::arc::PlanInput { arc, form: &form[..self.piece.form_len], structure_bars: &structure_bars[..sn], phrase_bars, seed: plan_seed(self.piece.seed, g.seed_key), length_bars };
        crate::arc::plan(&input, &mut self.arc);
        self.arc_key = key;
    }

    /// Begin the bar now starting: its own stream of choices and its own range of tags (the planned
    /// phrase's piece bar, else the bars since the score began).
    fn begin_bar(&mut self) {
        let (seed, base) = match self.planned {
            Some((_, start)) => {
                let b = start + self.bar_in_phrase as u64;
                (bar_seed(self.piece.seed, b), ((b as u32) << 12) & 0x7fff_f000)
            }
            None => {
                let b = self.bars_total.saturating_sub(1);
                (crate::piece::mix(crate::piece::mix(self.piece.seed, SALT_FREE_BAR), b), 0x8000_0000 | (((b as u32) << 12) & 0x7fff_f000))
            }
        };
        self.rng = Rng::new(seed);
        self.tag_base = base;
        self.tag_k = 0;
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

    /// The next tag in the bar's range (never 0, which means none).
    fn next_tag(&mut self) -> u32 {
        self.tag_k = (self.tag_k + 1) & 0xfff;
        if self.tag_k == 0 {
            self.tag_k = 1;
        }
        self.tag_base | self.tag_k
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
            sidechain: None,
            delay: None,
            gated: None,
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
        // A build's last beats: silence before the drop. The music lets go of everything it holds
        // (the drone and texture come back with the drop's phrase) and nothing is struck.
        let in_gap = now >= self.gap_from;
        if in_gap && !self.gap_done {
            self.gap_done = true;
            for h in [&mut self.pads, &mut self.choir, &mut self.bass, &mut self.drone, &mut self.texture] {
                for k in 0..h.n {
                    sink.note_off(h.tags[k]);
                }
                h.n = 0;
            }
            self.drone_root = -1;
        }
        // A tape stop: nothing new is struck while the tape runs down (what sounds slows with it).
        let in_gap = in_gap || self.in_tape_stop();
        // A build's snare roll, speeding up across the build: eighths for its first half, sixteenths
        // for the next quarter, 32nds for the last, louder as it goes; it stops for the silence.
        if let (Some(roll), false) = (self.part_section(res).filter(|s| s.build).and(self.structure(res).and_then(|st| st.roll)), in_gap) {
            let total = (self.bars_in_phrase * spb).max(1) as f32;
            let at = (self.bar_in_phrase * spb + step_of_bar) as f32 / total;
            let per_beat = sub as f32;
            // Steps per hit: an eighth note's worth, a sixteenth's, then two hits a sixteenth.
            let eighth = (per_beat / 2.0).max(1.0) as u32;
            let sixteenth = (per_beat / 4.0).max(1.0) as u32;
            let (every, doubled) = if at < 0.5 { (eighth, false) } else if at < 0.75 { (sixteenth, false) } else { (sixteenth, true) };
            if step_of_bar % every.max(1) == 0 {
                let vel = (roll.velocity * (0.35 + 0.65 * at) * self.plan.vel_scale.max(0.3)).min(1.0);
                let tag = self.next_tag();
                sink.note_on(NoteOn { inst: roll.inst, role: 255, midi: roll.midi as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: 1.0 });
                if doubled {
                    let half = (self.beat_len / per_beat as f64 * every as f64 * 0.5) as u64;
                    let tag = self.next_tag();
                    self.schedule(now + half, NoteOn { inst: roll.inst, role: 255, midi: roll.midi as f32, a4: self.steer.a4, vel: vel * 0.9, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: 1.0 });
                }
            }
        }
        // Drums.
        if eff.beat_felt && !in_gap {
            // A fill (rung E6): in a phrase's last bar (never a build's: the roll is its fill), each
            // fill line plays its pattern, and from the fill's first hit the lines that do not play
            // under fills rest.
            let last_bar = self.bar_in_phrase + 1 >= self.bars_in_phrase && !self.part_section(res).is_some_and(|s| s.build);
            let mut fill_from = usize::MAX;
            if last_bar {
                for d in eff.drums.iter().flatten() {
                    let Some(part) = drum_part(res, self.mood, d.genre, d.part).filter(|p| p.fill && !p.patterns.is_empty()) else { continue };
                    let pat = &part.patterns[self.plan.patterns[d.part as usize % 8] as usize % part.patterns.len()];
                    if let Some(first) = pat.iter().position(|&v| v > 0.0) {
                        // In grid steps of this bar.
                        fill_from = fill_from.min((first * spb as usize).div_ceil(pat.len().max(1)));
                    }
                }
            }
            let filling = step_of_bar as usize >= fill_from;
            for d in eff.drums.iter().flatten() {
                let Some(part) = drum_part(res, self.mood, d.genre, d.part) else { continue };
                // A part joins at its intensity, live on the beat (the taiko's doubled ostinato).
                if part.patterns.is_empty() || self.steer.intensity < part.min_intensity {
                    continue;
                }
                if (part.fill && !last_bar) || (filling && !part.fill && !part.under_fills) {
                    continue;
                }
                let pi = self.plan.patterns[d.part as usize % 8] as usize % part.patterns.len();
                let pat = &part.patterns[pi];
                if pat.is_empty() {
                    continue;
                }
                let idx = (step_of_bar as usize * pat.len()) / spb as usize;
                // A line of several notes (a fill down the toms): this step's.
                let midi = match part.pitch.get(pi).and_then(|p| p.get(idx.min(pat.len() - 1))) {
                    Some(&k) if k > 0 => part.notes.get(k as usize - 1).copied().unwrap_or(part.midi),
                    _ => part.midi,
                };
                let mut v = pat[idx.min(pat.len() - 1)];
                if eff.syncopation > 0.0 && self.rng.chance(eff.syncopation * 0.25) {
                    v = if v > 0.0 { v * 0.6 } else if self.step_in_beat % 2 == 1 { 0.3 } else { 0.0 };
                }
                if v > 0.0 {
                    // A kick that ducks: the sidechain roles dip under it (the composer knows the
                    // hit, so no detector listens for it).
                    if let (true, Some((roles, depth, attack, release))) = (part.ducks, eff.sidechain) {
                        sink.duck(roles, depth, attack, release);
                    }
                    let jitter = (self.rng.bipolar() * eff.humanize_ms * 0.001 * rate as f32) as i64;
                    let at = (now as i64 + jitter.max(0)) as u64;
                    let tag = self.next_tag();
                    // A gated line sends its share to the gated reverb (and opens its gate) where the
                    // genre has one.
                    let gate = if eff.gated.is_some() { part.gate } else { 0.0 };
                    let note = NoteOn { inst: part.inst, role: 255, midi: midi as f32, a4: self.steer.a4, vel: (v * part.velocity * d.weight * (0.6 + 0.4 * self.plan.vel_scale)).min(1.0), pan: 0.0, gate, hold_s: 0.0, tag, level: 1.0 };
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
        if let (Some(si), Some(pad), false) = (eff.pad_patterns, eff.roles[RoleKind::Pad.index()], in_gap) {
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
                        let strum = if eff.comping.strum_beats > 0.0 { (eff.comping.strum_beats as f64 * self.beat_len) as u64 } else { (eff.comping.strum_ms.max(0.0) as f64 * 0.001 * rate) as u64 };
                        let hold_s = (eff.comping.hold_beats.max(0.0) as f64 * self.beat_len / rate) as f32;
                        for k in 0..self.pad_voicing.n {
                            let tag = self.next_tag();
                            let midi = self.pad_voicing.notes[k];
                            let vel = ((0.55 + 0.35 * v + self.rng.range(-0.05, 0.05)) * self.plan.vel_scale).clamp(0.0, 1.0);
                            let note = NoteOn { inst: pad.inst, role: RoleKind::Pad.index() as u8, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s, tag, level: pad.level };
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
        if let (Some(si), Some(ch), false) = (eff.stab_patterns, eff.roles[RoleKind::Choir.index()], in_gap) {
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
                            sink.note_on(NoteOn { inst: ch.inst, role: RoleKind::Choir.index() as u8, midi: self.choir_voicing.notes[k] as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s, tag, level: ch.level });
                        }
                    }
                }
            }
        }
        // The bass on the genre's pattern.
        if let (Some(si), Some(bass), false) = (eff.bass_patterns, eff.roles[RoleKind::Bass.index()], in_gap) {
            let pats = bass_pats(res, self.mood, si);
            if !pats.is_empty() {
                let pat = &pats[self.plan.patterns[6] as usize % pats.len()];
                if !pat.is_empty() {
                    let c = pat[((step_of_bar as usize * pat.len()) / spb as usize).min(pat.len() - 1)];
                    let root = self.chord_root_pc(res);
                    let prev = if self.bass.n > 0 && !self.bass_fresh { self.bass.notes[0] } else { (bass.low + bass.high) / 2 };
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
                        sink.note_on(NoteOn { inst: bass.inst, role: RoleKind::Bass.index() as u8, midi: target as f32, a4: self.steer.a4, vel: 0.8 + self.rng.range(-0.08, 0.08), pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: bass.level });
                        self.bass = Held { notes: [target, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
                        self.bass_fresh = false;
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
            self.beats_total += 1;
            self.tempo_beats += 1;
            self.beat_start = self.tempo_origin + self.tempo_beats as f64 * self.beat_len;
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
        self.begin_bar();
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
                            self.schedule(at, NoteOn { inst: perc.inst, role: RoleKind::Percussion.index() as u8, midi: root as f32, a4: self.steer.a4, vel: v, pan, gate: 0.0, hold_s: 0.0, tag, level: perc.level });
                        }
                    }
                }
            }
        }
    }

    fn started_phrase(&self) -> bool {
        self.phrase_count > 0
    }

    /// The genre (or blend) over the mood playing, for a phrase; `rng` draws which genre's instrument a
    /// role takes in a blend.
    fn resolve_eff(&self, res: &Resolved, rng: &mut Rng) -> Eff {
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
        // The mood's own role, or one of its options as this piece drew (a calm mood's pad sung by
        // the four choir voices), and the roles that option rests.
        let mut mood_roles: [Option<EffRole>; 8] = [None; 8];
        let mut option_rests: [Option<[bool; 8]>; 8] = [None; 8];
        for k in 0..8 {
            mood_roles[k] = mood.roles[k].as_ref().map(base);
            let opts = mood.options.iter().filter(|o| o.0 == k);
            if mood.roles[k].is_none() || opts.clone().next().is_none() {
                continue;
            }
            let i = crate::piece::pick(self.piece.lead_draw[k], std::iter::once(mood.role_weights[k]).chain(opts.clone().map(|o| o.2)));
            if let Some(o) = i.checked_sub(1).and_then(|j| opts.clone().nth(j)) {
                mood_roles[k] = Some(base(&o.1));
                option_rests[k] = Some(o.3);
            }
        }
        for k in 0..8 {
            let pick = |s: Option<&crate::resolved::RGenre>, leads: &[Option<EffRole>; 8]| -> Option<EffRole> {
                if let Some(l) = leads[k] {
                    return Some(l);
                }
                let off = s.is_some_and(|s| s.roles_off[k]);
                let mut r = match (mood_roles[k], s.and_then(|s| s.roles_add[k].as_ref())) {
                    (_, Some(add)) => Some(base(add)),
                    (Some(m), None) if !off => Some(m),
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
                    (Some(x), Some(y)) => Some(if rng.chance(t) { y } else { x }),
                    (Some(x), None) => (t < 0.5).then_some(x),
                    (None, Some(y)) => (t >= 0.5).then_some(y),
                    (None, None) => None,
                }
            };
            if roles[k].is_some() && self.steer.intensity < mood.roles[k].as_ref().map(|m| m.min_intensity).unwrap_or(0.0) {
                roles[k] = None;
            }
        }
        // An option the mood's role plays as drawn (no genre lead, added role, silence or palette in
        // its place) rests the roles it names.
        let dom_genre = if t >= 0.5 { b } else { a };
        for k in 0..8 {
            let Some(rests) = option_rests[k] else { continue };
            let replaced = dom_genre.is_some_and(|s| s.roles_add[k].is_some() || s.roles_off[k] || s.palette[k].is_some() || s.leads.iter().any(|e| e.0 == k));
            if replaced || roles[k].is_none() {
                continue;
            }
            for (j, rest) in rests.iter().enumerate() {
                if *rest && j != k {
                    roles[j] = None;
                }
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
                // The kit this piece drew, of a genre with kits: only its parts (and the parts of
                // no kit) play.
                let kit = self.piece.kit(&s.kits);
                for (p, d) in s.drums.iter().enumerate().take(255) {
                    if k < 8 && (d.kits == 0 || kit.is_some_and(|i| d.kits & (1 << i.min(31)) != 0)) {
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
            // A decay or a low cut only one genre sets moves toward none (the mood's decay) with the blend.
            reverb_t60_s: match (ra.reverb_t60_s > 0.0, rb.reverb_t60_s > 0.0) {
                (true, true) => lerp(ra.reverb_t60_s, rb.reverb_t60_s),
                (true, false) => (t < 0.5).then_some(ra.reverb_t60_s).unwrap_or(0.0),
                (false, true) => (t >= 0.5).then_some(rb.reverb_t60_s).unwrap_or(0.0),
                (false, false) => 0.0,
            },
            reverb_low_cut_hz: lerp(ra.reverb_low_cut_hz, rb.reverb_low_cut_hz),
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
            sidechain: dom.and_then(|s| s.sidechain),
            delay: dom.and_then(|s| s.delay),
            gated: dom.and_then(|s| s.gated),
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
            eff.sidechain = None;
            eff.delay = None;
            eff.gated = None;
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
                    let n = NoteOn { inst, role: 254, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: 1.0 };
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
                    sink.note_on(NoteOn { inst, role: 254, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s: hold.max(0.05), tag, level: 1.0 });
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
                // A genre with a structure plays its sections in order as its form.
                match &gg.structure {
                    Some(st) => self.piece.choose_form(dominant, gg.seed_key, &st.form),
                    None => self.piece.choose_form(dominant, gg.seed_key, &gg.forms),
                }
            }
            if let Some(rules) = gg.motif_rules.as_ref().filter(|_| self.gen_src != dominant) {
                let mut r = Rng::new(crate::piece::mix(self.piece.seed, 0x707 ^ gg.seed_key));
                self.gen_n = crate::piece::make_motifs(rules, &mut r, &mut self.gen_motifs);
                self.gen_src = dominant;
            }
        }
        // The planned phrase (every calm phrase of a genre is planned: crate::arc): its section, its
        // stage, its energy. A danger mood plays outside the plan, and the plan picks up where it was.
        if let Some(gg) = g {
            self.ensure_plan(res, dominant, gg);
        }
        let index = self.piece.played;
        let planned = if calm && g.is_some() { self.arc.phrase(index) } else { None };
        self.planned = planned.map(|(p, _, start)| (p, start));
        if let Some((_, _, start)) = planned {
            self.form_bar = start;
        }
        // (letter, chorus, returning, plays its tune): a genre without an arc improvises every time
        // round after the first; an arc's tunes return in every round.
        let trivial = self.arc.trivial;
        let section = planned.map(|(p, round, _)| {
            let tune = (p.tune && (!trivial || round == 0)) || self.verbatim;
            let chorus = if trivial { round } else { p.chorus as u64 + round * ROUND_CHORUS as u64 };
            let returning = tune && (p.returning || round > 0);
            (p.letter, if tune { 0 } else { chorus }, returning, tune)
        });
        // The structure's section this phrase plays.
        self.part = match (g.and_then(|gg| gg.structure.as_ref()), planned) {
            (Some(st), Some((p, _, _))) if (p.part as usize) < st.sections.len() => Some((dominant, p.part)),
            _ => None,
        };
        let part_sec = self.part_section(res);
        self.gap_from = u64::MAX;
        self.gap_done = false;
        // A tape stop has just run out: the score silenced everything the tape held (crate::score),
        // so nothing is held now and this phrase strikes its chords, bass and drone afresh.
        if self.tape_to != u64::MAX && self.pos >= self.tape_to {
            for h in [&mut self.pads, &mut self.choir, &mut self.bass, &mut self.drone, &mut self.texture] {
                h.n = 0;
            }
            self.drone_root = -1;
        }
        self.tape_from = u64::MAX;
        self.tape_to = u64::MAX;
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
            self.home_adapted = !first;
        }
        if let Some((p, _, _)) = planned {
            // A planned phrase's key: the piece's home key (the seed's, unless a move of harmony
            // mid-piece re-keyed it), moved as the plan's stage says.
            if !self.home_adapted {
                self.home_key = crate::piece::pick(self.piece.key_draw, harm.keys.iter().map(|k| k.2));
            }
            self.key_idx = self.planned_key(harm, &p);
        } else if !harm_changed && self.phrase_count % 4 == 0 {
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
        let mut eff = match planned {
            Some((p, _, _)) => {
                let mut r = Rng::new(crate::piece::mix(phrase_seed(self.piece.seed, index), 0xb1e));
                let mut e = self.resolve_eff(res, &mut r);
                apply_stage(&mut e, &p, section.is_some_and(|s| s.3));
                e
            }
            None => {
                let mut r = self.rng;
                let e = self.resolve_eff(res, &mut r);
                self.rng = r;
                e
            }
        };
        if self.steer.tune.swing >= 0.0 {
            eff.swing = self.steer.tune.swing.clamp(0.5, 0.75);
        }
        // A structure's section plays only its roles, and only the drums that play in it.
        if let Some(sec) = part_sec {
            if let Some(on) = sec.roles {
                for (k, r) in eff.roles.iter_mut().enumerate() {
                    if !on[k] {
                        *r = None;
                    }
                }
            }
            for d in eff.drums.iter_mut() {
                if let Some(x) = d {
                    if drum_part(res, self.mood, x.genre, x.part).is_some_and(|p| p.sections & (1u32 << sec.letter.min(31)) == 0) {
                        *d = None;
                    }
                }
            }
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
        } else if planned.is_some() {
            self.piece_tempo(eff.tempo)
        } else {
            // The draw happens whatever the person chose, so a tempo change leaves the rest alone.
            let drawn = self.rng.range(eff.tempo.0, eff.tempo.1.max(eff.tempo.0));
            self.steer.tune.tempo_for(drawn, eff.tempo)
        }
        .clamp(20.0, 240.0);
        // A new tempo counts its beats from this bar line.
        let beat_len = self.rate * 60.0 / self.tempo as f64;
        if beat_len != self.beat_len {
            self.tempo_origin = self.beat_start;
            self.tempo_beats = 0;
            self.beat_len = beat_len;
        }
        let now_s = self.seconds() as f32;
        let window = m.no_repeat_s;
        let n_motifs = self.motif_count(res);
        let start_chord = if harm_changed { harm.start } else { self.chord };
        let plan = match (planned, section) {
            (Some((p, _, _)), Some((_, chorus, _, tune))) => {
                // A section of the piece: its chords from its own seed (the same letter is the same
                // chords all piece), its tune where the plan states or returns it; elsewhere the riffs
                // are improvised over the same chords (the second half reharmonised for this time),
                // from the phrase's own seed, drawn again while they sound like a phrase of the same
                // letter in the mood's window (a check made from the plan, so a seek makes it too).
                let sec = self.planned_section(res, m, harm, harm_src, &eff, &p, index, chorus, tune);
                let key_pc = self.key_pc;
                if tune {
                    let mut r = Rng::new(phrase_seed(self.piece.seed, index));
                    self.make_plan(m, harm, n_motifs, start_chord, false, eff, Some((sec.0, Some(sec.1))), &mut r, key_pc)
                } else {
                    let mut chosen = None;
                    let mut last = Plan::default();
                    for a in 0..ATTEMPTS {
                        let mut r = Rng::new(crate::piece::mix(phrase_seed(self.piece.seed, index), a));
                        last = self.make_plan(m, harm, n_motifs, start_chord, false, eff, Some((sec.0, None)), &mut r, key_pc);
                        if self.verbatim || !self.sounds_like_recent(res, m, harm, harm_src, index, &p, &sec.0, last.hash) {
                            chosen = Some(last);
                            break;
                        }
                    }
                    chosen.unwrap_or(last)
                }
            }
            _ => {
                // Outside the plan (a danger mood): every phrase walked afresh, re-made while it
                // repeats one played within the window.
                let mut r = self.rng;
                let key_pc = self.key_pc;
                let mut plan = self.make_plan(m, harm, n_motifs, start_chord, !harm_changed, eff, None, &mut r, key_pc);
                for _ in 0..24 {
                    let seen = self.history.iter().any(|&(h, t)| h == plan.hash && now_s - t < window);
                    if !seen {
                        break;
                    }
                    plan = self.make_plan(m, harm, n_motifs, start_chord, !harm_changed, eff, None, &mut r, key_pc);
                }
                self.rng = r;
                self.history[self.hist_w] = (plan.hash, now_s);
                self.hist_w = (self.hist_w + 1) % HISTORY;
                plan
            }
        };
        if planned.is_some() {
            self.piece.played += 1;
        }
        self.section = section.map(|(l, c, r, _)| (l, c, r));
        self.plan = plan;
        self.bars_in_phrase = (0..plan.n).map(|i| plan.bars[i] as u32).sum::<u32>().max(1);
        self.bar_in_phrase = 0;
        self.slot = 0;
        self.bars_into_chord = 0;
        self.phrase_count += 1;
        self.bass_fresh = true;
        // The phrase's first bar: its own stream and tags from here (its striking below included).
        self.begin_bar();
        // The plan's energy for this phrase moves its velocities.
        let (stage, energy) = planned.map(|(p, _, _)| (p.stage, p.energy)).unwrap_or((255, 1.0));
        self.plan.vel_scale *= crate::arc::velocity_of(energy);
        let (sec_letter, chorus, returning) = section.filter(|s| s.0 != FREE).map(|(l, c, r, _)| (l + 1, c.min(u32::MAX as u64) as u32, r)).unwrap_or((0, 0, false));
        sink.event(Event::Phrase { hash: plan.hash, at_s: self.seconds(), mood: self.mood as u16, key_pc: self.key_pc, section: sec_letter, chorus, returning, stage, energy });
        // A structure's section: its energy, a build's riser and silence, a drop's impact.
        if let (Some(sec), Some((_, index))) = (part_sec, self.part) {
            self.plan.vel_scale *= sec.energy;
            let beats = (self.bars_in_phrase * eff.beats_per_bar.max(1)) as f64;
            sink.event(Event::Structure { index, build: sec.build, drop: sec.drop, bars: self.bars_in_phrase, at_s: self.seconds() });
            let st = self.structure(res).map(|s| (s.riser, s.impact, s.gap_beats, s.reverse_cymbal, s.downlifter));
            // A tape stop over the section's last beats (at most a bar): it stands in for a build's
            // silence.
            let stop = (sec.tape_stop_beats as f64).min(eff.beats_per_bar.max(1) as f64).min(beats - 1.0).max(0.0);
            if stop > 0.0 {
                self.tape_from = self.pos + ((beats - stop) * self.beat_len) as u64;
                self.tape_to = self.pos + (beats * self.beat_len) as u64;
            }
            if let Some((riser, impact, gap, reverse, downlifter)) = st {
                // A reversed cymbal into the next section, if it swells in: it starts its `beats`
                // before this section ends and stops dead on the next downbeat.
                let next = self.arc.phrase(self.piece.played).map(|(p, _, _)| p.part as usize);
                let next_swells = next.and_then(|i| self.structure(res).and_then(|s| s.sections.get(i))).is_some_and(|s| s.swell);
                if let (true, Some(r)) = (next_swells, reverse) {
                    let lead = (r.beats as f64).clamp(0.5, beats);
                    let at = self.pos + ((beats - lead) * self.beat_len) as u64;
                    let tag = self.next_tag();
                    let hold_s = (lead * self.beat_len / self.rate) as f32;
                    self.schedule(at, NoteOn { inst: r.inst, role: 254, midi: r.midi as f32, a4: self.steer.a4, vel: r.velocity, pan: 0.0, gate: 0.0, hold_s, tag, level: 1.0 });
                }
                // A downlifter falls as this section begins (out of a drop).
                if let (true, Some(h)) = (sec.fall, downlifter) {
                    let tag = self.next_tag();
                    let hold_s = (h.beats.max(0.0) as f64 * self.beat_len / self.rate) as f32;
                    sink.note_on(NoteOn { inst: h.inst, role: 254, midi: h.midi as f32, a4: self.steer.a4, vel: h.velocity, pan: 0.0, gate: 0.0, hold_s, tag, level: 1.0 });
                }
                if sec.build {
                    let gap = if stop > 0.0 { 0.0 } else { (gap as f64).min(beats - 1.0).max(0.0) };
                    self.gap_from = if stop > 0.0 { u64::MAX } else { self.pos + ((beats - gap) * self.beat_len) as u64 };
                    if let Some(r) = riser {
                        // The riser ends where the silence begins, starting `beats` before it.
                        let lead = (r.beats as f64).clamp(1.0, beats - gap);
                        let at = self.pos + ((beats - gap - lead) * self.beat_len) as u64;
                        let tag = self.next_tag();
                        let hold_s = (lead * self.beat_len / self.rate) as f32;
                        self.schedule(at, NoteOn { inst: r.inst, role: 254, midi: r.midi as f32, a4: self.steer.a4, vel: r.velocity, pan: 0.0, gate: 0.0, hold_s, tag, level: 1.0 });
                    }
                }
                if let (true, Some(h)) = (sec.drop, impact) {
                    let tag = self.next_tag();
                    sink.note_on(NoteOn { inst: h.inst, role: 254, midi: h.midi as f32, a4: self.steer.a4, vel: h.velocity, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: 1.0 });
                }
            }
        }
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
                sink.note_on(NoteOn { inst: d.inst, role: RoleKind::Drone.index() as u8, midi: want_root as f32, a4: self.steer.a4, vel: 0.9, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: d.level });
                self.drone = Held { notes: [want_root, 0, 0, 0, 0, 0, 0, 0], tags: [tag, 0, 0, 0, 0, 0, 0, 0], n: 1 };
            }
            self.drone_root = want_root;
        }
        // The texture runs under everything while the genre has one.
        match (eff.roles[RoleKind::Texture.index()], self.texture.n) {
            (Some(t), 0) => {
                let tag = self.next_tag();
                let mid = (t.low + t.high) / 2;
                sink.note_on(NoteOn { inst: t.inst, role: RoleKind::Texture.index() as u8, midi: mid as f32, a4: self.steer.a4, vel: 0.9, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: t.level });
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
    #[allow(clippy::too_many_arguments)]
    fn make_plan(&self, m: &RMood, harm: &RHarmony, n_motifs: usize, start: usize, continuing: bool, eff: Eff, sec: Option<(Section, Option<Rng>)>, rng: &mut Rng, key_pc: i32) -> Plan {
        let seed = self.piece.seed;
        let walk = |rng: &mut Rng, cur: usize, cadence: bool| crate::piece::walk(&harm.moves, &harm.cadence, rng, seed, cur, cadence);
        let mut p = Plan::default();
        let mut theme: Option<Rng> = None;
        if let Some((s, tune)) = sec {
            p.n = s.n.min(MAX_SLOTS);
            p.chords[..p.n].copy_from_slice(&s.chords[..p.n]);
            p.bars[..p.n].copy_from_slice(&s.bars[..p.n]);
            theme = tune;
        } else {
            let bars_total = m.phrase_bars[rng.int(0, m.phrase_bars.len() as i32 - 1) as usize].clamp(1, 64);
            let (lo, hi) = eff.harmony.chord_bars.unwrap_or(m.chord_bars);
            let (lo, hi) = (lo.max(1), hi.max(lo.max(1)));
            let mut cur = if continuing { walk(rng, start, false) } else { start };
            let mut used = 0u32;
            while used < bars_total && p.n < MAX_SLOTS {
                let mut bars = rng.int(lo as i32, hi as i32) as u32;
                if used + bars > bars_total || p.n == MAX_SLOTS - 1 {
                    bars = bars_total - used;
                }
                p.chords[p.n] = cur as u16;
                p.bars[p.n] = bars.min(255) as u8;
                p.n += 1;
                used += bars;
                if used < bars_total {
                    let last = used + lo > bars_total || used + hi >= bars_total;
                    cur = walk(rng, cur, last);
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
                None => draw_motif(rng, &eff, n_motifs, &mut last_call),
            };
            p.choir[i] = rng.chance(dc);
        }
        p.pulse = rng.chance(dp);
        for k in 0..8 {
            p.patterns[k] = rng.int(0, 7) as u8;
        }
        p.vel_scale = match eff.dynamics {
            Some((lo, hi)) => rng.range(lo, hi.max(lo)),
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
        h = fnv(h, key_pc as u64);
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

    /// The key planned phrase `p` plays in (an index in `harm`'s keys): the home key; for a section
    /// other than A, a second key when this piece's seed says its bridge moves (about a third of
    /// pieces); a stage away, that second key; a stage lifted, the harmony's key nearest that many
    /// semitones above home.
    fn planned_key(&self, harm: &RHarmony, p: &PlanPhrase) -> usize {
        let n = harm.keys.len();
        let home = self.home_key.min(n.saturating_sub(1));
        if n < 2 {
            return home;
        }
        let away = || crate::piece::pick(crate::piece::unit(crate::piece::mix(self.piece.seed, 0xb2 + p.letter as u64)), harm.keys.iter().enumerate().map(|(i, k)| if i == home { 0.0 } else { k.2 }));
        match p.key {
            crate::arc::KeyMove::Home if p.letter != 0 && p.letter != FREE && self.piece.bridge_moves < 0.35 => away(),
            crate::arc::KeyMove::Home => home,
            crate::arc::KeyMove::Away => away(),
            crate::arc::KeyMove::Up(st) => {
                let want = (harm.keys[home].0 + st).rem_euclid(12);
                (0..n).filter(|&i| i != home).min_by_key(|&i| {
                    let d = (harm.keys[i].0 - want).rem_euclid(12);
                    d.min(12 - d)
                })
                .unwrap_or(home)
            }
        }
    }

    /// Planned phrase `index`'s section (`p`, in the key and with the genre's rules `eff`): its chords
    /// from its section's seed (the same letter, the same chords), the second half reharmonised for an
    /// improvised time (`chorus`), and the section's own generator after its chords, for its tune.
    #[allow(clippy::too_many_arguments)]
    fn planned_section(&self, res: &Resolved, m: &RMood, harm: &RHarmony, harm_src: u16, eff: &Eff, p: &PlanPhrase, index: u64, chorus: u64, tune: bool) -> (Section, Rng) {
        // Keyed by the harmony's id, never its place in the list (BUG-299).
        let harmony_id = if harm_src == MOOD_SRC { 0x1000 ^ m.seed_key } else { res.genres.get(harm_src as usize).map(|x| x.seed_key).unwrap_or(0) };
        let base = if p.letter == FREE { crate::piece::mix(self.piece.section_seed(harmony_id, FREE), index) } else { self.piece.section_seed(harmony_id, p.letter) };
        let mut sec_rng = Rng::new(base);
        let chord_bars = eff.harmony.chord_bars.unwrap_or(m.chord_bars);
        let seed = self.piece.seed;
        let mut sec = section_chords(harm, &mut sec_rng, seed, p.letter, p.bars, chord_bars, p.end_home);
        if !tune && chorus > 0 && p.letter != FREE {
            // The first half as stated, the second half reharmonised (its own seed for this time), so
            // the section stays recognisable and no two minutes of an hour are alike.
            let mut alt_rng = Rng::new(crate::piece::mix(base, chorus));
            let alt = section_chords(harm, &mut alt_rng, seed, p.letter, p.bars, chord_bars, p.end_home);
            sec = crate::piece::splice(&sec, &alt, sec.total_bars() / 2);
        }
        (sec, sec_rng)
    }

    /// Whether an improvised plan (`hash`, its section `sec`) of planned phrase `index` sounds like a
    /// phrase of the same letter within the mood's no-repeat window: one in the same key with the same
    /// chords, any of whose own attempts has the same fingerprint. Made from the plan alone (each
    /// earlier phrase recomputed from its seeds), so the same answer comes however the phrase was
    /// reached; the cost is a section's chords for each phrase of the letter in the window.
    #[allow(clippy::too_many_arguments)]
    fn sounds_like_recent(&self, res: &Resolved, m: &RMood, harm: &RHarmony, harm_src: u16, index: u64, p: &PlanPhrase, sec: &Section, hash: u64) -> bool {
        let Some((_, _, start)) = self.arc.phrase(index) else { return false };
        let bar_s = self.beat_len / self.rate * self.eff().beats_per_bar.max(1) as f64;
        let window_bars = (m.no_repeat_s as f64 / bar_s.max(1e-3)).ceil() as u64;
        let key_now = self.key_idx;
        let trivial = self.arc.trivial;
        let mut j = index;
        while j > 0 {
            j -= 1;
            let Some((pj, round, sj)) = self.arc.phrase(j) else { break };
            if start.saturating_sub(sj) > window_bars {
                break;
            }
            let tune_j = (pj.tune && (!trivial || round == 0)) || self.verbatim;
            if pj.letter != p.letter || tune_j || pj.letter == FREE {
                continue;
            }
            if self.planned_key(harm, &pj) != key_now {
                continue;
            }
            let chorus_j = if trivial { round } else { pj.chorus as u64 + round * ROUND_CHORUS as u64 };
            let mut r = Rng::new(crate::piece::mix(phrase_seed(self.piece.seed, j), 0xb1e));
            let mut eff_j = self.resolve_eff(res, &mut r);
            apply_stage(&mut eff_j, &pj, false);
            let (sec_j, _) = self.planned_section(res, m, harm, harm_src, &eff_j, &pj, j, chorus_j, false);
            if sec_j.n != sec.n || sec_j.chords[..sec.n] != sec.chords[..sec.n] || sec_j.bars[..sec.n] != sec.bars[..sec.n] {
                continue;
            }
            let n_motifs = self.motif_count(res);
            for b in 0..ATTEMPTS {
                let mut r = Rng::new(crate::piece::mix(phrase_seed(self.piece.seed, j), b));
                if self.make_plan(m, harm, n_motifs, 0, false, eff_j, Some((sec_j, None)), &mut r, self.key_pc).hash == hash {
                    return true;
                }
            }
        }
        false
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
            // Placed nearest the register's reference voicing (register_ref): a voicing depends on its
            // chord alone, never on one long past, so a seek voices it the same.
            let led = self.pad_voicing;
            let notes = self.voicing(0, pad.voices as usize, pad.low, pad.high, &led.notes[..led.n], &pcs[..n], root_pc);
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
                        sink.note_on(NoteOn { inst: pad.inst, role: RoleKind::Pad.index() as u8, midi: midi as f32, a4: self.steer.a4, vel, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: pad.level * if midi < 48 { 0.6 } else { 1.0 } });
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
                let notes = self.voicing(1, ch.voices as usize, ch.low, ch.high, &led.notes[..led.n], &pcs[..n], root_pc);
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
                let notes = self.voicing(1, ch.voices as usize, ch.low, ch.high, &led.notes[..led.n], &pcs[..n], root_pc);
                self.choir_voicing = Held { notes, tags: [0; 8], n: ch.voices as usize };
                sink.event(Event::Voicing { role: RoleKind::Choir.index() as u8, notes, n: ch.voices, at_s });
                let mut next = Held::default();
                for k in 0..ch.voices as usize {
                    let midi = notes[k];
                    if let Some(j) = (0..prev.n).find(|&j| prev.notes[j] == midi && !next.tags[..next.n].contains(&prev.tags[j])) {
                        next.tags[next.n] = prev.tags[j];
                    } else {
                        let tag = self.next_tag();
                        sink.note_on(NoteOn { inst: ch.inst, role: RoleKind::Choir.index() as u8, midi: midi as f32, a4: self.steer.a4, vel: 0.8, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: ch.level });
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
            let prev = if self.bass.n > 0 && !self.bass_fresh { self.bass.notes[0] } else { (b.low + b.high) / 2 };
            self.bass_fresh = false;
            let target = nearest_within(prev, &[root_pc, (root_pc + 7) % 12], 5).unwrap_or_else(|| nearest_pc_in(root_pc, b.low, b.high, prev));
            if self.bass.n == 0 || self.bass.notes[0] != target {
                for k in 0..self.bass.n {
                    sink.note_off(self.bass.tags[k]);
                }
                let tag = self.next_tag();
                sink.note_on(NoteOn { inst: b.inst, role: RoleKind::Bass.index() as u8, midi: target as f32, a4: self.steer.a4, vel: 0.85, pan: 0.0, gate: 0.0, hold_s: 0.0, tag, level: b.level });
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
                    self.schedule(at, NoteOn { inst: r.inst, role: role_idx as u8, midi: midi as f32, a4: self.steer.a4, vel, pan, gate: 0.0, hold_s: (len * self.beat_len as f32 / self.rate as f32) * motif.hold[k], tag, level: r.level });
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

/// A planned phrase's stage over the genre's setup: only the roles the stage lets play, the drums
/// only where it says, and the riffs, the choir and the heartbeat as often as its energy says (a
/// phrase playing its section's TUNE keeps the tune's own densities, so the theme returns note for
/// note at any energy; its velocities still follow the energy).
fn apply_stage(eff: &mut Eff, p: &PlanPhrase, tune: bool) {
    for (k, r) in eff.roles.iter_mut().enumerate() {
        if p.roles & (1 << k) == 0 {
            *r = None;
        }
    }
    if !p.drums {
        eff.drums = [None; 8];
    }
    let f = if tune { 1.0 } else { crate::arc::density_of(p.energy) };
    for k in [RoleKind::Melody, RoleKind::Bells, RoleKind::Choir, RoleKind::Percussion] {
        if let Some(r) = eff.roles[k.index()].as_mut() {
            r.density = (r.density * f).min(1.0);
        }
    }
}

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
    voice_lead_within(prev, n, pcs, root_pc, lo, hi, (-5, 5), None)
}

/// [`voice_lead`] with each voice's candidates the chord tones within `window` semitones of where it
/// was (the nearest within a fourth when none is), and, with `pull`, each voice's distance from that
/// reference voicing added to the cost (a voicing drawn toward a register).
#[allow(clippy::too_many_arguments)]
pub fn voice_lead_within(prev: &[i32], n: usize, pcs: &[i32], root_pc: i32, lo: i32, hi: i32, window: (i32, i32), pull: Option<&[i32; 8]>) -> [i32; 8] {
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
        for (a, b) in [window, (-5, 5)] {
            if cn[v] > 0 {
                break;
            }
            for d in a..=b {
                let m = sorted[v] + d;
                if pcs.contains(&m.rem_euclid(12)) && cn[v] < 11 {
                    cand[v][cn[v]] = m;
                    cn[v] += 1;
                }
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
            if let Some(r) = pull {
                cost += (cur[v] - r[v]).abs();
            }
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
