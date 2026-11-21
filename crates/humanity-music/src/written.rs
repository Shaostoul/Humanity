//! A WRITTEN PIECE, timed and played (docs/design/score-player.md sections 7, 10 and 11).
//!
//! [`Timeline::from_mei`] turns a read score (crate::mei: exact positions in whole notes) into
//! seconds through its tempo map, once, on the music's build thread: every note's start and length,
//! every syllable's time (the sing-along's hook: [`Timeline::syllable_at`]), every bar line.
//!
//! [`WrittenPlayer`] plays a timeline in place of the composer, through the same score
//! (`ScoreEngine::written`): each block it sends the notes now due to the score's sink as `NoteOn`s,
//! sample accurate, each held for its written length (a little less, so repeated notes stay two
//! notes), on the instrument the piece's index names for its staff, at the person's tuning
//! (`NoteOn::a4`; the voice plays `tuning::note_hz`). When the piece ends it rests, then plays again.
//! Nothing here allocates once made: the audio thread only walks the timeline.

use std::sync::Arc;

use crate::composer::Sink;
use crate::mei::{Frac, MeiScore, MeiTempo, WordPos};
use crate::voice::NoteOn;

/// The tempo before a file's first tempo mark, when its index line gives none: quarter notes a minute.
pub const DEFAULT_QPM: f64 = 120.0;

/// How many times its written length a fermata's moment lasts (score-player.md section 16, rung 2).
pub const FERMATA_HOLD: f64 = 2.0;

/// What a tenuto (held its full length) leaves between it and the next note, seconds.
const TENUTO_GAP_S: f64 = 0.005;

/// Seconds that `whole` whole notes last at `qpm` quarter notes a minute.
fn secs(whole: Frac, qpm: f64) -> f64 {
    whole.to_f64() * 4.0 * 60.0 / qpm
}

/// The tempo map: points in score time (whole notes), each with its tempo and the seconds at it.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoMap {
    points: Vec<(Frac, f64, f64)>,
}

impl TempoMap {
    /// From a score's tempo marks; `default_qpm` before the first (a mark at the very start replaces it).
    pub fn new(tempos: &[MeiTempo], default_qpm: f64) -> TempoMap {
        TempoMap::with_holds(tempos, default_qpm, &[])
    }

    /// The same, with each fermata's moment (`holds`: start and written length, not overlapping)
    /// played [`FERMATA_HOLD`] times as long: time slows there, so every note sounding is held and
    /// everything after it comes later.
    pub fn with_holds(tempos: &[MeiTempo], default_qpm: f64, holds: &[(Frac, Frac)]) -> TempoMap {
        let mut marks = tempos.to_vec();
        marks.sort_by(|a, b| a.at.cmp(&b.at));
        let mut base: Vec<(Frac, f64)> = vec![(Frac::ZERO, default_qpm.clamp(5.0, 1000.0))];
        for t in marks {
            match base.last_mut() {
                Some(last) if last.0 == t.at => last.1 = t.qpm,
                _ => base.push((t.at, t.qpm)),
            }
        }
        let mut cuts: Vec<Frac> = base.iter().map(|p| p.0).chain(holds.iter().flat_map(|&(at, len)| [at, at.add(len)])).collect();
        cuts.sort();
        cuts.dedup();
        let pts: Vec<(Frac, f64)> = cuts
            .into_iter()
            .map(|at| {
                let q = base[base.partition_point(|p| p.0 <= at).max(1) - 1].1;
                let held = holds.iter().any(|&(h, len)| at >= h && at < h.add(len));
                (at, if held { q / FERMATA_HOLD } else { q })
            })
            .collect();
        let mut points = Vec::with_capacity(pts.len());
        let mut s = 0.0;
        for (i, &(at, qpm)) in pts.iter().enumerate() {
            if i > 0 {
                let (pat, pq) = pts[i - 1];
                s += secs(at.sub(pat), pq);
            }
            points.push((at, qpm, s));
        }
        TempoMap { points }
    }

    fn point(&self, at: Frac) -> (Frac, f64, f64) {
        let i = self.points.partition_point(|p| p.0 <= at).max(1) - 1;
        self.points[i]
    }

    /// The seconds at a position in the score.
    pub fn seconds(&self, at: Frac) -> f64 {
        let (p, q, s) = self.point(at);
        s + secs(at.sub(p), q)
    }

    /// The tempo at a position, quarter notes a minute.
    pub fn qpm(&self, at: Frac) -> f64 {
        self.point(at).1
    }
}

/// A note in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedNote {
    pub at_s: f64,
    pub dur_s: f64,
    /// How long the player holds it before it lets go (its articulation, else [`hold_s`]'s rule).
    pub hold_s: f32,
    pub midi: f32,
    pub vel: f32,
    pub staff: u16,
}

/// A bar line in seconds: its start, its length and its meter's beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedBar {
    pub at_s: f64,
    pub len_s: f64,
    pub beats: u8,
    pub qpm: f64,
}

/// A syllable in seconds: when its note sounds, which note (an index into [`Timeline::notes`] and
/// the note's `xml:id`), and the words.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedSyllable {
    pub at_s: f64,
    pub note: usize,
    pub note_id: String,
    pub verse: u16,
    pub text: String,
    pub wordpos: WordPos,
    pub con: Option<char>,
}

/// A written piece in seconds: what the player walks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    /// By start.
    pub notes: Vec<TimedNote>,
    /// In the order sung.
    pub syllables: Vec<TimedSyllable>,
    pub bars: Vec<TimedBar>,
    pub length_s: f64,
}

impl Timeline {
    /// Time a read score at its tempo marks (`default_qpm` before the first).
    pub fn from_mei(score: &MeiScore, default_qpm: f64) -> Timeline {
        let map = TempoMap::with_holds(&score.tempos, default_qpm, &score.fermatas);
        let notes: Vec<TimedNote> = score
            .notes
            .iter()
            .map(|n| {
                let at_s = map.seconds(n.at);
                let dur_s = map.seconds(n.at.add(n.dur)) - at_s;
                let held = match n.hold {
                    Some(f) if f >= 0.999 => (dur_s - TENUTO_GAP_S).max(0.01) as f32,
                    Some(f) => (dur_s * f as f64).max(0.01) as f32,
                    None => hold_s(dur_s),
                };
                TimedNote { at_s, dur_s, hold_s: held, midi: n.midi, vel: n.vel, staff: n.staff }
            })
            .collect();
        let syllables = score
            .syllables
            .iter()
            .filter_map(|s| {
                let n = notes.get(s.note)?;
                Some(TimedSyllable { at_s: n.at_s, note: s.note, note_id: s.note_id.clone(), verse: s.verse, text: s.text.clone(), wordpos: s.wordpos, con: s.con })
            })
            .collect();
        let bars = score
            .bars
            .iter()
            .map(|b| {
                let at_s = map.seconds(b.at);
                TimedBar { at_s, len_s: map.seconds(b.at.add(b.len)) - at_s, beats: b.meter.0.clamp(1, 255) as u8, qpm: map.qpm(b.at) }
            })
            .collect();
        Timeline { notes, syllables, bars, length_s: map.seconds(score.end) }
    }

    /// The syllable of `verse` being sung at `t` seconds: the last one begun (None before the first).
    pub fn syllable_at(&self, t: f64, verse: u16) -> Option<&TimedSyllable> {
        self.syllables.iter().filter(|s| s.verse == verse).take_while(|s| s.at_s <= t).last()
    }

    /// The bar playing at `t` seconds (its index; the last bar after the end).
    pub fn bar_at(&self, t: f64) -> Option<usize> {
        if self.bars.is_empty() {
            return None;
        }
        Some(self.bars.partition_point(|b| b.at_s <= t).max(1) - 1)
    }
}

/// What plays a staff: an instrument (its index in the resolved library), the role it counts as on
/// the Music page (its meter and colour; 255: none), its level as an amplitude and its pan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffVoice {
    pub staff: u16,
    pub inst: u16,
    pub role: u8,
    pub level: f32,
    pub pan: f32,
}

/// How long a note of `dur_s` is held: a little less than written (a tenth, at most 50 ms), so two
/// notes of the same pitch in a row are heard as two.
pub fn hold_s(dur_s: f64) -> f32 {
    let gap = (dur_s * 0.1).min(0.05);
    (dur_s - gap).max(0.01) as f32
}

/// Plays a timeline in place of the composer (score.rs).
pub struct WrittenPlayer {
    timeline: Arc<Timeline>,
    voices: Vec<StaffVoice>,
    rate: f64,
    a4: f32,
    /// Samples into this time through, the next note to send, and the rest after the end.
    pos: u64,
    next: usize,
    rest_s: f64,
    /// Times through finished.
    loops: u64,
    tag: u32,
    /// The piece's level, dB: what the score's level follows instead of the mood's.
    pub level_db: f32,
    /// Drum staves a sound style mapped key by key (crate::style): the staff, and per key the
    /// instrument, the note it plays at and its level (`u16::MAX`: no instrument, the key is skipped).
    drums: Vec<(u16, Box<[(u16, f32, f32); 128]>)>,
}

impl WrittenPlayer {
    /// A player of `timeline` with `voices` (a staff no voice names is silent) at `rate` Hz and A4 =
    /// `a4`, resting `rest_s` seconds after each time through.
    pub fn new(timeline: Arc<Timeline>, voices: Vec<StaffVoice>, rate: f32, a4: f32, rest_s: f32, level_db: f32) -> WrittenPlayer {
        WrittenPlayer { timeline, voices, rate: rate.max(1.0) as f64, a4, pos: 0, next: 0, rest_s: rest_s.max(0.0) as f64, loops: 0, tag: 1, level_db, drums: Vec::new() }
    }

    /// Staff `staff` is a drum track a sound style mapped: each key in `hits` (the key, the
    /// instrument's index, the note it plays at, its level as an amplitude) plays on its own
    /// instrument. Made here, before the player reaches the audio thread.
    pub fn set_drums(&mut self, staff: u16, hits: &[(u8, u16, f32, f32)]) {
        let mut table = Box::new([(u16::MAX, 0.0f32, 0.0f32); 128]);
        for &(k, inst, midi, level) in hits {
            if let Some(slot) = table.get_mut(k as usize) {
                *slot = (inst, midi, level);
            }
        }
        self.drums.retain(|(s, _)| *s != staff);
        self.drums.push((staff, table));
    }

    pub fn timeline(&self) -> &Arc<Timeline> {
        &self.timeline
    }

    fn sample_of(&self, t_s: f64) -> u64 {
        (t_s * self.rate).round().max(0.0) as u64
    }

    /// One time through and its rest, in samples (at least one).
    fn cycle(&self) -> u64 {
        self.sample_of(self.timeline.length_s + self.rest_s).max(1)
    }

    /// The sound card's rate changed: the place in the piece is kept in seconds.
    pub fn set_rate(&mut self, rate: f32) {
        let rate = rate.max(1.0) as f64;
        self.pos = (self.pos as f64 * rate / self.rate) as u64;
        self.rate = rate;
    }

    /// The tuning changed (each note after this one follows it).
    pub fn set_a4(&mut self, a4: f32) {
        self.a4 = a4;
    }

    /// Carry on from the start of bar `bar` (a score rebuilt mid-piece takes over where it was).
    pub fn seek_bar(&mut self, bar: usize) {
        let Some(b) = self.timeline.bars.get(bar) else { return };
        let at = self.sample_of(b.at_s);
        self.pos = at;
        self.next = self.timeline.notes.partition_point(|n| self.sample_of(n.at_s) < at);
    }

    /// Send every note due now.
    pub fn process(&mut self, sink: &mut dyn Sink) {
        let cycle = self.cycle();
        if self.pos >= cycle {
            self.pos -= cycle;
            self.next = 0;
            self.loops += 1;
        }
        while let Some(n) = self.timeline.notes.get(self.next).copied() {
            if self.sample_of(n.at_s) > self.pos {
                break;
            }
            self.next += 1;
            let Some(v) = self.voices.iter().find(|v| v.staff == n.staff).copied() else { continue };
            // A mapped drum staff: the key's own instrument, at its own pitch and level.
            let (inst, midi, level) = match self.drums.iter().find(|(s, _)| *s == n.staff) {
                Some((_, table)) => match table[(n.midi.round().clamp(0.0, 127.0)) as usize] {
                    (u16::MAX, _, _) => continue,
                    (i, m, l) => (i, m, v.level * l),
                },
                None => (v.inst, n.midi, v.level),
            };
            self.tag = self.tag.wrapping_add(1).max(1);
            sink.note_on(NoteOn { inst, role: v.role, midi, a4: self.a4, vel: n.vel, pan: v.pan, gate: 0.0, hold_s: n.hold_s, tag: self.tag, level });
        }
    }

    /// Samples until the next note, or the end of the rest (at least 1).
    pub fn until_next(&self) -> usize {
        let target = match self.timeline.notes.get(self.next) {
            Some(n) => self.sample_of(n.at_s),
            None => self.cycle(),
        };
        target.saturating_sub(self.pos).clamp(1, usize::MAX as u64) as usize
    }

    /// Time moves on by `n` samples.
    pub fn advance(&mut self, n: usize) {
        self.pos += n as u64;
    }

    /// Seconds into this time through.
    pub fn seconds(&self) -> f64 {
        self.pos as f64 / self.rate
    }

    /// Times through finished.
    pub fn loops(&self) -> u64 {
        self.loops
    }

    /// The bar playing (its index in the piece), and bars begun since the piece first started.
    pub fn bar(&self) -> usize {
        self.timeline.bar_at(self.seconds()).unwrap_or(0)
    }

    pub fn bars_begun(&self) -> u64 {
        self.loops * self.timeline.bars.len() as u64 + self.bar() as u64 + 1
    }

    /// Where in the bar the music is, in beats, the bar's beats and its tempo (the Music page's beat
    /// pulse and the status).
    pub fn beat(&self) -> (f32, u8, f64) {
        let t = self.seconds();
        match self.timeline.bar_at(t).and_then(|i| self.timeline.bars.get(i)) {
            Some(b) if t < b.at_s + b.len_s => (((t - b.at_s) / b.len_s.max(1e-6)) as f32 * b.beats as f32, b.beats, b.qpm),
            Some(b) => (0.0, b.beats, b.qpm),
            None => (0.0, 4, DEFAULT_QPM),
        }
    }
}
