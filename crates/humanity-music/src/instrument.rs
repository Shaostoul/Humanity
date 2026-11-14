//! Instruments as data (docs/design/music.md, section 6): `data/music/instruments/<id>.ron`, one
//! file per instrument. The KINDS are the engine's code (synth, choir, modal, membrane, FM, plucked,
//! noise, sampler); every instrument is a file, so a new sound is a new file, never code
//! (infinite-of-x).
//!
//! **The sampler seam (rung 6).** Recorded instruments come from the free instrument library (CC0
//! sample packs, installed from Settings > Music > Instruments by the instrument-library work).
//! This crate defines what a loaded recording IS ([`SampleZone`], [`SampleSet`]) and how the engine
//! asks for one ([`SampleBank`]); the pack loader decodes files into a `SampleSet` and hands the
//! engine a [`SampleLibrary`] keyed by instrument id. An instrument of kind `Sampler` names a
//! `fallback` synthesised instrument, which plays whenever its pack is not installed, so music
//! always plays. Retuning is by playback rate (`rate = f_wanted / recorded_hz`), so A = 432 Hz
//! reaches recorded notes too, with nothing retuned on disk.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// One instrument.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instrument {
    /// Stable id: the file name, and what moods and genres name.
    pub id: String,
    /// Its name, by key in data/i18n/en.json (a community file may give plain words instead).
    #[serde(default)]
    pub words: String,
    /// What makes the sound.
    pub kind: InstrumentKind,
    /// Its level against the other instruments, dB.
    #[serde(default)]
    pub level_db: f32,
    /// How much of it goes to the reverb, 0..1.
    #[serde(default = "default_send")]
    pub reverb_send: f32,
    /// Each note's pan is chosen within +- this (0 = centre, 1 = anywhere).
    #[serde(default)]
    pub pan_spread: f32,
}

fn default_send() -> f32 {
    0.35
}

/// What makes an instrument's sound.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InstrumentKind {
    /// Oscillators through a filter: pads, drones, basses, soft leads.
    Synth(SynthSpec),
    /// Formant synthesis: a wordless choir ("ah", "oh") as a pad.
    Choir(ChoirSpec),
    /// A sum of decaying partials at given ratios: bells, celesta, glass, STEEL PAN.
    Modal(ModalSpec),
    /// A struck skin: membrane modes with a falling pitch and a noise strike: DEEP DRUMS (taiko,
    /// frame drum, timpani-like), kicks.
    Membrane(MembraneSpec),
    /// Two-operator FM: electric piano, soft bells.
    Fm(FmSpec),
    /// Extended Karplus-Strong: plucked strings, an upright bass.
    Plucked(PluckedSpec),
    /// Filtered noise with an optional tonal body and crackle: snares, hats, brushes, vinyl crackle.
    Noise(NoiseSpec),
    /// Recordings from an installed pack, with a synthesised fallback (the seam above).
    Sampler(SamplerSpec),
}

/// An envelope's times (seconds) and sustain level.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EnvSpec {
    pub attack_s: f32,
    #[serde(default)]
    pub decay_s: f32,
    #[serde(default = "one")]
    pub sustain: f32,
    pub release_s: f32,
}

fn one() -> f32 {
    1.0
}

fn default_q() -> f32 {
    0.707
}

/// An oscillator's waveform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
}

/// One oscillator of a synth.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OscSpec {
    pub wave: Wave,
    /// Fine detune, cents.
    #[serde(default)]
    pub detune_cents: f32,
    /// Coarse offset, semitones (a drone's fifth is 7, its octave 12).
    #[serde(default)]
    pub semitones: f32,
    #[serde(default = "one")]
    pub level: f32,
    /// -1 left .. 1 right.
    #[serde(default)]
    pub pan: f32,
}

/// A filter's response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterKind {
    LowPass,
    BandPass,
    HighPass,
}

/// The shape an LFO sweeps in (docs/design/music.md section 7d, the bass toolkit): a sine, a
/// triangle, a rising or falling ramp, a square, or a new random value each cycle (sample and hold).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    RampUp,
    RampDown,
    Square,
    SampleHold,
}

/// A modulator in time with the music (the wobble's sweep, a growl's FM depth, a vowel's morph):
/// cycles per beat (2 is eighth notes, 3 the eighth-note triplet, 4 sixteenths), or one rate per bar
/// in turn (`rates`: the wobble's rate changing bar by bar), in a shape. In time with the beat clock
/// (every note in the bar sweeps in step) unless `restart` starts it afresh at each note.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BeatLfo {
    #[serde(default)]
    pub per_beat: f32,
    #[serde(default)]
    pub rates: Vec<f32>,
    #[serde(default)]
    pub shape: LfoShape,
    #[serde(default)]
    pub restart: bool,
}

/// A distortion stage (docs/design/music.md section 7d): a curve driven by `drive`, mixed with the
/// clean signal (`mix`, parallel distortion), then bit and sample-rate reduction.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ShaperSpec {
    #[serde(default)]
    pub curve: ShaperCurve,
    /// How hard the curve is driven, 0 .. 1 (a gain of 1 to 21 into it).
    #[serde(default)]
    pub drive: f32,
    /// How much of the shaped signal is heard, 0 (none) .. 1 (all of it).
    #[serde(default = "one")]
    pub mix: f32,
    /// Bit depth to reduce to (0: none; 6 is gritty, 12 the DX7's).
    #[serde(default)]
    pub bits: f32,
    /// Hold each sample this many times (1: none; 4 at 48 kHz is a 12 kHz sample rate).
    #[serde(default = "one")]
    pub downsample: f32,
}

/// A waveshaper's curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaperCurve {
    /// A smooth saturation (tanh): warm.
    #[default]
    Soft,
    /// Clipped flat at full scale: harsh.
    Hard,
    /// Folded back from full scale (a wavefolder): metallic, more harmonics the harder it is driven.
    Fold,
    /// Soft on one side, harder on the other: even harmonics, a growl's grit.
    Asymmetric,
}

/// One of the vowels a formant filter sings (the bass voice's formants, docs/design/music.md 7d).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
}

/// A formant ("talking") filter: the sound through a vowel's three resonances, morphing along
/// `vowels` in time with the music (`lfo`'s sweep 0..1 walks the list and back), mixed with the dry
/// sound by `mix`. `shift` moves every formant (0.8: a bigger throat; 1.2 a smaller).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormantSpec {
    pub vowels: Vec<Vowel>,
    #[serde(default)]
    pub lfo: BeatLfo,
    #[serde(default = "one")]
    pub mix: f32,
    #[serde(default = "one")]
    pub shift: f32,
}

/// A feedback comb filter: the sound plus itself a moment later, again and again, so it rings at
/// one pitch and its overtones (the "wet", metallic edge). Tuned to the note (`key_ratio` times its
/// frequency) or to `delay_ms`; `lfo` sweeps the delay by `sweep` of itself (a flanger's swoosh).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombSpec {
    #[serde(default)]
    pub key_ratio: f32,
    #[serde(default)]
    pub delay_ms: f32,
    /// -0.95 .. 0.95 (negative: odd overtones only, hollow).
    pub feedback: f32,
    #[serde(default = "half")]
    pub mix: f32,
    #[serde(default)]
    pub lfo: BeatLfo,
    #[serde(default)]
    pub sweep: f32,
}

/// A phaser: four all-pass stages whose turning point an LFO sweeps between `low_hz` and `high_hz`,
/// mixed with the dry sound, so moving notches pass through it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaserSpec {
    pub low_hz: f32,
    pub high_hz: f32,
    #[serde(default)]
    pub feedback: f32,
    #[serde(default = "half")]
    pub mix: f32,
    #[serde(default)]
    pub lfo: BeatLfo,
}

/// A protected sub (docs/design/music.md 7d): a clean sine at the note (`octave` octaves down: -1 an
/// octave under), added AFTER every distortion and filter so it stays round and centred, at `level`;
/// the processed sound above it is high-passed at `split_hz` so the two do not fight.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SubSpec {
    pub level: f32,
    #[serde(default)]
    pub octave: f32,
    #[serde(default)]
    pub split_hz: f32,
}

/// What follows a synth's or an FM voice's filter, in order: the shaper, the formant filter, the
/// comb, the phaser, then the protected sub. Any of them may be absent; with any present the voice
/// is processed in mono (a bass sits in the centre).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VoiceFx {
    #[serde(default)]
    pub shaper: Option<ShaperSpec>,
    #[serde(default)]
    pub formant: Option<FormantSpec>,
    #[serde(default)]
    pub comb: Option<CombSpec>,
    #[serde(default)]
    pub phaser: Option<PhaserSpec>,
    #[serde(default)]
    pub sub: Option<SubSpec>,
}

impl VoiceFx {
    /// Whether anything is in the chain.
    pub fn any(&self) -> bool {
        self.shaper.is_some() || self.formant.is_some() || self.comb.is_some() || self.phaser.is_some() || self.sub.is_some()
    }
}

/// A filter and what moves its cutoff.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterSpec {
    pub kind: FilterKind,
    pub cutoff_hz: f32,
    #[serde(default = "default_q")]
    pub q: f32,
    /// The note's frequency times this is added to the cutoff (brighter high notes).
    #[serde(default)]
    pub key_track: f32,
    /// A slow sweep: rate (Hz) and depth (Hz).
    #[serde(default)]
    pub lfo_hz: f32,
    #[serde(default)]
    pub lfo_depth_hz: f32,
    /// A sweep tied to the tempo instead (a wobble bass): cycles per beat (0 = off).
    #[serde(default)]
    pub lfo_per_beat: f32,
    /// The envelope times this is added to the cutoff (a pluck's "pew").
    #[serde(default)]
    pub env_hz: f32,
    /// Two stages in series (24 dB an octave, the wobble's slope) instead of one (12 dB).
    #[serde(default = "one_u8")]
    pub stages: u8,
    /// The tempo LFO's shape, its rate per bar in turn (cycles per beat; over `lfo_per_beat`), and
    /// whether it starts afresh at each note (else it runs in time with the bar).
    #[serde(default)]
    pub lfo_shape: LfoShape,
    #[serde(default)]
    pub lfo_rates: Vec<f32>,
    #[serde(default)]
    pub lfo_restart: bool,
    /// Drive into the filter, 0 .. 1 (a resonant filter pushed harder squelches).
    #[serde(default)]
    pub drive: f32,
}

fn one_u8() -> u8 {
    1
}

impl FilterSpec {
    /// The tempo LFO as a [`BeatLfo`] view: its rates, shape and restart (None: no tempo LFO).
    pub fn tempo_lfo(&self) -> Option<(f32, &[f32], LfoShape, bool)> {
        (self.lfo_per_beat > 0.0 || !self.lfo_rates.is_empty()).then_some((self.lfo_per_beat, self.lfo_rates.as_slice(), self.lfo_shape, self.lfo_restart))
    }
}

/// A synth: 1 to 4 oscillators, a filter, an envelope, vibrato, tremolo and drive.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynthSpec {
    pub oscillators: Vec<OscSpec>,
    #[serde(default)]
    pub filter: Option<FilterSpec>,
    pub envelope: EnvSpec,
    /// Vibrato: rate (Hz) and depth (cents).
    #[serde(default)]
    pub vibrato: Option<(f32, f32)>,
    /// Tremolo or breathing: rate (Hz) and depth (0..1).
    #[serde(default)]
    pub tremolo: Option<(f32, f32)>,
    /// Soft saturation, 0 (clean) .. 1.
    #[serde(default)]
    pub drive: f32,
    /// Glide: each note slides from the pitch the role last played, over about this long (seconds;
    /// 0: none). A bass's or a lead's portamento.
    #[serde(default)]
    pub glide_s: f32,
    /// The bass toolkit's chain after the filter (shaper, formant, comb, phaser, protected sub).
    #[serde(default)]
    pub fx: VoiceFx,
}

/// A formant choir.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChoirSpec {
    /// Singers per note, 1 to 3 (an ensemble, detuned a few cents apart).
    #[serde(default = "two")]
    pub singers: u8,
    pub envelope: EnvSpec,
    /// Vibrato: rate (Hz) and depth (cents); it fades in over the first 1.5 s.
    #[serde(default = "default_choir_vibrato")]
    pub vibrato: (f32, f32),
    /// Breath noise in the source, 0..1.
    #[serde(default = "default_breath")]
    pub breath: f32,
}

fn two() -> u8 {
    2
}
fn default_choir_vibrato() -> (f32, f32) {
    (5.2, 6.0)
}
fn default_breath() -> f32 {
    0.06
}

/// Decaying partials: (ratio to the note's frequency, amplitude, seconds to fall 60 dB).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModalSpec {
    pub partials: Vec<(f32, f32, f32)>,
    /// The strike's rise, seconds.
    #[serde(default = "default_modal_attack")]
    pub attack_s: f32,
    /// A burst of filtered noise at the strike (a mallet's contact), 0..1, and how long it lasts.
    #[serde(default)]
    pub strike_noise: f32,
    #[serde(default = "default_strike_ms")]
    pub strike_ms: f32,
    /// Higher notes ring shorter: decays scale by (note frequency / 440)^-this.
    #[serde(default)]
    pub decay_pitch_scale: f32,
    /// A held note let go dies this many times faster (a damped bar); 1 = it rings on.
    #[serde(default = "one")]
    pub damping_on_release: f32,
}

fn default_modal_attack() -> f32 {
    0.004
}
fn default_strike_ms() -> f32 {
    12.0
}

/// A struck membrane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MembraneSpec {
    /// The skin's modes: (ratio to the fundamental, amplitude, seconds to fall 60 dB).
    pub modes: Vec<(f32, f32, f32)>,
    /// The pitch starts this fraction sharp (tension rises when the skin is struck) ...
    #[serde(default)]
    pub pitch_drop: f32,
    /// ... and falls back with this time constant, seconds.
    #[serde(default = "default_drop_s")]
    pub pitch_drop_s: f32,
    /// The strike's noise: level 0..1, its low-pass cutoff (Hz) and decay (seconds).
    #[serde(default)]
    pub noise: f32,
    #[serde(default = "default_noise_hz")]
    pub noise_hz: f32,
    #[serde(default = "default_noise_decay")]
    pub noise_decay_s: f32,
    /// A soft mallet's rise, seconds.
    #[serde(default = "default_membrane_attack")]
    pub attack_s: f32,
}

fn default_drop_s() -> f32 {
    0.06
}
fn default_noise_hz() -> f32 {
    600.0
}
fn default_noise_decay() -> f32 {
    0.05
}
fn default_membrane_attack() -> f32 {
    0.006
}

/// Two-operator FM.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FmSpec {
    /// The modulator's frequency over the carrier's.
    pub ratio: f32,
    /// The modulation index at the strike, and where it settles (and how fast, seconds).
    pub index: f32,
    #[serde(default)]
    pub index_sustain: f32,
    #[serde(default = "default_index_decay")]
    pub index_decay_s: f32,
    pub envelope: EnvSpec,
    /// An extra decaying partial (an electric piano's tine): (ratio, amplitude, seconds to fall 60 dB).
    #[serde(default)]
    pub tine: Option<(f32, f32, f32)>,
    /// The modulation index moved in time with the music (a growl: more FM is buzzier): the LFO,
    /// and how far it adds to the index (0 .. this, by its sweep).
    #[serde(default)]
    pub index_lfo: Option<BeatLfo>,
    #[serde(default)]
    pub index_lfo_depth: f32,
    /// Glide (seconds) and the chain after the operators, as a synth's.
    #[serde(default)]
    pub glide_s: f32,
    #[serde(default)]
    pub fx: VoiceFx,
}

fn default_index_decay() -> f32 {
    0.6
}

/// A plucked string.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluckedSpec {
    /// Seconds for a low note to fall 60 dB.
    pub decay_s: f32,
    /// How bright the pluck starts, 0 (thumb) .. 1 (pick).
    #[serde(default = "half")]
    pub brightness: f32,
    /// A resonant body under the string: centre (Hz) and level (0 = none).
    #[serde(default)]
    pub body_hz: f32,
    #[serde(default)]
    pub body_level: f32,
    /// A note let go dies this many times faster (a palm on the string).
    #[serde(default = "default_pluck_damp")]
    pub damping_on_release: f32,
}

fn half() -> f32 {
    0.5
}
fn default_pluck_damp() -> f32 {
    6.0
}

/// Filtered noise, an optional tonal body, and crackle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoiseSpec {
    pub filter: FilterSpec,
    pub envelope: EnvSpec,
    /// The filtered noise's level (0 for a crackle that is only pops and hiss).
    #[serde(default = "one")]
    pub noise_level: f32,
    /// A tonal body (a snare's shell): frequency (Hz, 0 = none), level and decay (seconds).
    #[serde(default)]
    pub tone_hz: f32,
    #[serde(default)]
    pub tone_level: f32,
    #[serde(default = "default_tone_decay")]
    pub tone_decay_s: f32,
    /// Pops per second (vinyl crackle; 0 = none) and a steady hiss level.
    #[serde(default)]
    pub crackle_per_s: f32,
    #[serde(default)]
    pub hiss: f32,
}

fn default_tone_decay() -> f32 {
    0.12
}

/// A recorded instrument from an installed pack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SamplerSpec {
    /// The pack that holds it ("core" ships with the app; others are downloads).
    pub pack: String,
    /// Its folder under the pack (data/music/samples/<folder>/ for the core set).
    pub folder: String,
    /// The synthesised instrument that plays when the pack is not installed.
    pub fallback: String,
    /// The envelope over each recording (a sustained recording is looped while held).
    #[serde(default = "default_sampler_env")]
    pub envelope: EnvSpec,
    /// The fallback's index among the score's instruments (filled by `Resolved::new`, never written
    /// in a file): a note the recordings cannot play (an instrument with exact keys, such as a Hang
    /// with only its nine notes) plays on it instead of going silent.
    #[serde(skip)]
    pub fallback_index: Option<u16>,
}

fn default_sampler_env() -> EnvSpec {
    EnvSpec { attack_s: 0.002, decay_s: 0.0, sustain: 1.0, release_s: 0.5 }
}

// --------------------------------------------------------------------------------------------
// The sampler seam: what the instrument library's loader fills.
// --------------------------------------------------------------------------------------------

/// A recording's frames are kept as 16-bit samples (CD resolution: the noise of the rounding sits
/// 96 dB under full scale), half the memory of 32-bit floats, so the core set's recordings fit
/// music.md section 14's 64 MB (BUG-266). A voice reads them times this.
pub const SAMPLE_SCALE: f32 = 1.0 / 32768.0;

/// Decoded frames (-1..1) as the 16-bit frames a `SampleZone` keeps.
pub fn frames16(frames: impl Iterator<Item = [f32; 2]>) -> Vec<[i16; 2]> {
    frames.map(|[l, r]| [sample16(l), sample16(r)]).collect()
}

/// One decoded sample (-1..1) as 16 bits, rounded: the one rounding every decode uses.
#[inline]
pub fn sample16(x: f32) -> i16 {
    (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

/// One recording: stereo frames at their own rate, 16-bit ([`SAMPLE_SCALE`]; shared, so a bank and
/// the voice reading it never copy them), the frequency of the note in it (at the library's own tuning: a sample library
/// recorded at A = 440 has its A4 at 440 Hz here, and the engine plays it 31.77 cents slower at
/// A = 432; 0 for an unpitched hit such as a drum, played at its recorded rate), the notes and
/// velocity it covers, and an optional loop for sustained notes.
#[derive(Clone, Debug)]
pub struct SampleZone {
    pub frames: Arc<Vec<[i16; 2]>>,
    pub rate: f32,
    pub recorded_hz: f32,
    pub low_note: i32,
    pub high_note: i32,
    /// The velocity it was played at, 0..1 (velocity layers).
    pub velocity: f32,
    /// A loop (start, end) in frames, for notes held longer than the recording.
    pub loop_range: Option<(usize, usize)>,
    /// 0: `loop_range` is seamless as recorded (the pipeline found it), played as it always was.
    /// Otherwise a CROSSFADED loop of this many frames: a sustained recording made without loop
    /// points (a string section's 8 s note) is held past its end by fading, over its last frames
    /// before the loop's end, from itself into the frames before the loop's start, then going on
    /// from the start (equal power), so a chord held longer than the recording never falls silent
    /// (BUG-266; src/instrument_packs/score.rs sets it for the parts that sustain).
    pub loop_xfade: usize,
    /// A long recording STREAMED from disk (music.md section 23, "Memory"): `frames` holds only its
    /// first part (the attack), and a note reads the rest from the stream as it plays. None: `frames`
    /// is the whole recording. Never set with a loop (a streamed note plays straight through).
    pub stream: Option<crate::stream::ZoneStream>,
}

impl SampleZone {
    /// The memory its frames hold, bytes (16-bit stereo; a streamed zone's attack only).
    pub fn memory(&self) -> usize {
        self.frames.len() * 4
    }
}

/// How the engine asks a loaded instrument for a recording. `Send + Sync` because the score
/// renders on the audio thread; the pack loader keeps the bank alive (the engine holds an `Arc`, and
/// a voice holds only indices), so no recording is ever freed on the audio thread.
pub trait SampleBank: Send + Sync {
    /// The recording to play for `midi` at `velocity`; `round_robin` counts the notes played, so a
    /// bank with several takes of a note can rotate them.
    fn pick(&self, midi: i32, velocity: f32, round_robin: u32) -> Option<usize>;
    /// A recording by the index `pick` gave.
    fn zone(&self, index: usize) -> &SampleZone;
    /// The memory its recordings hold, bytes (the Dev page's music memory).
    fn memory(&self) -> usize {
        0
    }
}

/// The plain bank: a list of recordings, the nearest covering one chosen by note, then velocity.
#[derive(Clone, Debug, Default)]
pub struct SampleSet {
    pub zones: Vec<SampleZone>,
    /// Only the keys its zones cover (a Hang's nine notes: its makers ask that the others not be
    /// made by pitching neighbours); any other key picks nothing and the note plays on the fallback.
    pub exact_keys: bool,
}

impl SampleBank for SampleSet {
    fn pick(&self, midi: i32, velocity: f32, round_robin: u32) -> Option<usize> {
        // Covering zones first; else the nearest by note. Among equals, the nearest velocity; among
        // takes equally near, rotate by round_robin.
        let dist = |z: &SampleZone| {
            if midi < z.low_note {
                z.low_note - midi
            } else if midi > z.high_note {
                midi - z.high_note
            } else {
                0
            }
        };
        let best_note = self.zones.iter().map(dist).min()?;
        if self.exact_keys && best_note > 0 {
            return None;
        }
        let best_vel = self
            .zones
            .iter()
            .filter(|z| dist(z) == best_note)
            .map(|z| ((z.velocity - velocity).abs() * 1000.0) as i32)
            .min()?;
        // Counted, then the nth taken: no list built (this runs at a note's start on the audio thread).
        let is_candidate = |z: &SampleZone| dist(z) == best_note && ((z.velocity - velocity).abs() * 1000.0) as i32 == best_vel;
        let count = self.zones.iter().filter(|z| is_candidate(z)).count();
        let nth = round_robin as usize % count.max(1);
        self.zones.iter().enumerate().filter(|(_, z)| is_candidate(z)).nth(nth).map(|(i, _)| i)
    }

    fn zone(&self, index: usize) -> &SampleZone {
        &self.zones[index]
    }

    fn memory(&self) -> usize {
        self.zones.iter().map(SampleZone::memory).sum()
    }
}

/// The installed recordings, by instrument id (empty: every sampled instrument plays its fallback).
pub type SampleLibrary = HashMap<String, Arc<dyn SampleBank>>;
