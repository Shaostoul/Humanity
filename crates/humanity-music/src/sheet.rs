//! THE SHEET: a piece written down (docs/design/music-page.md section 9). The Music page's piano
//! roll, its chord symbols and section letters, its words for the piece ("D harmonic minor, AABA"),
//! and the event hash that proves a favourite plays the same piece, all come from here.
//!
//! The composer is run DRY (no voices, no sound) over the same seed, genre, mood and tuning the
//! score plays, through the same loop the score runs it in (blocks of [`crate::render::BLOCK`]
//! frames, chunks of at most 64 samples cut at each decision), so every note lands on the very
//! sample the score starts it on. Minutes of music transcribe in milliseconds. Not on the audio
//! thread: this allocates (the page's thread, the tests).

use crate::composer::{Composer, Event, Sink};
use crate::resolved::Resolved;
use crate::score::MusicContext;
use crate::tuning::Mode;
use crate::voice::NoteOn;

/// The score's inner chunk (score.rs `CHUNK`): the transcription cuts time where the score does.
const CHUNK: usize = 64;

/// One note as the score plays it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetNote {
    /// When it starts, seconds from the piece's start, and how long it sounds (held notes: until
    /// their release; a note still held when the transcription ends: to its end).
    pub at_s: f64,
    pub dur_s: f32,
    /// The MIDI pitch (fractional for a glide's start), the role (`mood::RoleKind::ALL` index; 254:
    /// the danger stinger and riser), the instrument and the velocity.
    pub midi: f32,
    pub role: u8,
    pub inst: u16,
    pub vel: f32,
}

/// One chord as the score plays it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetChord {
    pub at_s: f64,
    pub beats: f32,
    /// Its root's pitch class and its pitch classes as a 12-bit mask.
    pub root: i8,
    pub pcs: u16,
}

/// A phrase beginning: the section of the piece's form it plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSection {
    pub at_s: f64,
    /// Its letter + 1 (1 = A; 0: the genre has no form), which time through the form, and whether
    /// the section is returning (its tune again).
    pub letter: u8,
    pub chorus: u32,
    pub returning: bool,
    /// The key and mode it is in, and its tempo.
    pub key_pc: i32,
    pub mode: Mode,
    pub tempo_bpm: f32,
}

/// A piece written down.
#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub notes: Vec<SheetNote>,
    pub chords: Vec<SheetChord>,
    pub sections: Vec<SheetSection>,
    /// Each bar's start, seconds (the composer's own bar lines), and the beats a bar has.
    pub bars: Vec<f64>,
    pub beats_per_bar: u32,
    /// The form's letters (0 = A), and per role the genre's lead entry the piece plays (an index
    /// in the dominant genre's `leads`; `tune::PIECE`: none).
    pub form: Vec<u8>,
    pub leads: [u16; 8],
    /// How long was transcribed, seconds, and the hash of every note-on and note-off with its
    /// sample: two transcriptions with the same hash composed the same piece, note for note.
    pub seconds: f64,
    pub hash: u64,
}

/// A note of the piano roll: its place in the bar, in beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollNote {
    pub midi: f32,
    pub role: u8,
    /// Beats from the bar's start, and its length in beats.
    pub start_beat: f32,
    pub len_beats: f32,
    pub vel: f32,
}

/// FNV-1a over a number's bytes (the event hash).
fn fnv(h: u64, x: u64) -> u64 {
    let mut h = h;
    for b in x.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// The sink the dry run writes into.
struct Recorder {
    pos: u64,
    rate: f64,
    sheet: Sheet,
    /// Notes waiting for their release: (tag, index in `notes`).
    open: Vec<(u32, usize)>,
    phrase_seen: bool,
}

impl Sink for Recorder {
    fn note_on(&mut self, n: NoteOn) {
        let at_s = self.pos as f64 / self.rate;
        let mut h = fnv(self.sheet.hash, self.pos);
        for x in [1u64, n.role as u64, n.inst as u64, n.midi.to_bits() as u64, n.vel.to_bits() as u64, n.hold_s.to_bits() as u64, n.tag as u64] {
            h = fnv(h, x);
        }
        self.sheet.hash = h;
        let i = self.sheet.notes.len();
        self.sheet.notes.push(SheetNote { at_s, dur_s: n.hold_s.max(0.0), midi: n.midi, role: n.role, inst: n.inst, vel: n.vel });
        if n.hold_s <= 0.0 {
            self.open.push((n.tag, i));
        }
    }

    fn note_off(&mut self, tag: u32) {
        self.sheet.hash = fnv(fnv(fnv(self.sheet.hash, self.pos), 2), tag as u64);
        if let Some(k) = self.open.iter().position(|(t, _)| *t == tag) {
            let (_, i) = self.open.swap_remove(k);
            let note = &mut self.sheet.notes[i];
            note.dur_s = (self.pos as f64 / self.rate - note.at_s).max(0.0) as f32;
        }
    }

    fn event(&mut self, e: Event) {
        match e {
            Event::Chord { root, pcs, at_s, beats, .. } => self.sheet.chords.push(SheetChord { at_s, beats, root, pcs }),
            Event::Phrase { at_s, key_pc, section, chorus, returning, .. } => {
                self.sheet.sections.push(SheetSection { at_s, letter: section, chorus, returning, key_pc, mode: Mode::Ionian, tempo_bpm: 0.0 });
                self.phrase_seen = true;
            }
            _ => {}
        }
    }
}

/// Write down `seconds` of the piece `seed` makes in context `ctx` (its mood, genres, intensity,
/// tuning: what the score is steered with), at `rate` Hz, from the very start.
pub fn transcribe(res: &Resolved, seed: u64, ctx: &MusicContext, rate: f32, seconds: f64) -> Sheet {
    let mut comp = Composer::new(rate, seed, crate::score::steer_of(ctx));
    let mut rec = Recorder { pos: 0, rate: rate as f64, sheet: Sheet::default(), open: Vec::new(), phrase_seen: false };
    let end = (seconds * rate as f64) as u64;
    let mut bars_seen = 0u64;
    let block = crate::render::BLOCK as u64;
    let mut pos = 0u64;
    while pos < end {
        // One block as the score renders it: decide, then render up to the next decision (at most
        // a chunk), until the block is done.
        let block_end = (pos + block).min(end);
        while pos < block_end {
            rec.pos = pos;
            comp.process(res, &mut rec);
            if rec.phrase_seen {
                rec.phrase_seen = false;
                if let Some(s) = rec.sheet.sections.last_mut() {
                    s.mode = comp.mode();
                    s.tempo_bpm = comp.tempo();
                }
            }
            if comp.bars() > bars_seen {
                bars_seen = comp.bars();
                rec.sheet.bars.push(pos as f64 / rate as f64);
            }
            let n = ((block_end - pos) as usize).min(comp.until_next()).min(CHUNK);
            comp.advance(n);
            pos += n as u64;
        }
    }
    let mut sheet = rec.sheet;
    for (_, i) in rec.open {
        let note = &mut sheet.notes[i];
        note.dur_s = (seconds - note.at_s).max(0.0) as f32;
    }
    sheet.beats_per_bar = comp.beats_per_bar();
    sheet.form = comp.piece_form().to_vec();
    let dominant = if ctx.blend >= 0.5 { ctx.genre_b } else { ctx.genre_a };
    if let Some(g) = res.genres.get(dominant as usize) {
        sheet.leads = comp.lead_picks(g);
    } else {
        sheet.leads = [crate::tune::PIECE; 8];
    }
    sheet.seconds = seconds;
    sheet
}

impl Sheet {
    /// The bar playing at `t` seconds (its index in `bars`), if any has begun.
    pub fn bar_at(&self, t: f64) -> Option<usize> {
        self.bars.iter().rposition(|&b| b <= t + 1e-9)
    }

    /// Bar `bar`'s start and end, seconds (the last bar ends where the transcription does).
    pub fn bar_span(&self, bar: usize) -> Option<(f64, f64)> {
        let a = *self.bars.get(bar)?;
        let b = self.bars.get(bar + 1).copied().unwrap_or(self.seconds);
        Some((a, b))
    }

    /// The piano roll's notes for bar `bar`: every note that STARTS in it (a note held over from the
    /// bar before belongs to that bar), its start and length in beats of this bar.
    pub fn bar_notes(&self, bar: usize) -> Vec<RollNote> {
        let Some((a, b)) = self.bar_span(bar) else { return Vec::new() };
        let beat_s = (b - a) / self.beats_per_bar.max(1) as f64;
        self.notes
            .iter()
            .filter(|n| n.at_s >= a - 1e-9 && n.at_s < b - 1e-9)
            .map(|n| RollNote { midi: n.midi, role: n.role, start_beat: ((n.at_s - a) / beat_s) as f32, len_beats: (n.dur_s as f64 / beat_s) as f32, vel: n.vel })
            .collect()
    }

    /// The chord sounding at `t` seconds.
    pub fn chord_at(&self, t: f64) -> Option<&SheetChord> {
        self.chords.iter().rev().find(|c| c.at_s <= t + 1e-9)
    }

    /// The section playing at `t` seconds.
    pub fn section_at(&self, t: f64) -> Option<&SheetSection> {
        self.sections.iter().rev().find(|s| s.at_s <= t + 1e-9)
    }

    /// Whether the piece spells its notes with flats (a key on the flat side of the circle).
    pub fn flats(&self) -> bool {
        self.sections.first().is_some_and(|s| key_uses_flats(s.key_pc, s.mode))
    }
}

/// Whether a key is spelled with flats: F, B flat, E flat, A flat, D flat and G flat major, and the
/// minor keys a minor third under them (D, G, C, F, B flat, E flat minor).
pub fn key_uses_flats(key_pc: i32, mode: Mode) -> bool {
    let major_pc = if mode.is_minor() { (key_pc + 3).rem_euclid(12) } else { key_pc.rem_euclid(12) };
    matches!(major_pc, 5 | 10 | 3 | 8 | 1 | 6)
}

const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLATS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// A pitch class's letter name, spelled with sharps or flats.
pub fn pc_name(pc: i32, flats: bool) -> &'static str {
    let i = pc.rem_euclid(12) as usize;
    if flats {
        FLATS[i]
    } else {
        SHARPS[i]
    }
}

/// The chord qualities a symbol is named from: the intervals above the root (a 12-bit mask) and
/// the suffix a lead sheet writes. Exact matches first; the commonplace jazz and pop spellings.
const QUALITIES: [(u16, &str); 16] = [
    (0b000010010001, ""),     // 0 4 7
    (0b000010001001, "m"),    // 0 3 7
    (0b000001001001, "dim"),  // 0 3 6
    (0b000100010001, "aug"),  // 0 4 8
    (0b010010010001, "7"),    // 0 4 7 10
    (0b100010010001, "maj7"), // 0 4 7 11
    (0b010010001001, "m7"),   // 0 3 7 10
    (0b010001001001, "m7b5"), // 0 3 6 10
    (0b001001001001, "dim7"), // 0 3 6 9
    (0b001010001001, "m6"),   // 0 3 7 9
    (0b001010010001, "6"),    // 0 4 7 9
    (0b100010001001, "mMaj7"), // 0 3 7 11
    (0b000010100001, "sus4"), // 0 5 7
    (0b000010000101, "sus2"), // 0 2 7
    (0b010010100001, "7sus4"), // 0 5 7 10
    (0b010001010001, "7b5"),  // 0 4 6 10
];

/// A chord's symbol from its root and its pitch classes: "Dm6", "A7", "Bb7", "C#dim7". A set of
/// notes no quality names exactly is named by its triad (the root's third and fifth) with "(+)".
pub fn chord_symbol(root: i8, pcs: u16, flats: bool) -> String {
    let r = root as i32;
    let mut rel = 0u16;
    for pc in 0..12 {
        if pcs & (1 << pc) != 0 {
            rel |= 1 << ((pc - r).rem_euclid(12));
        }
    }
    let name = pc_name(r, flats);
    if let Some((_, q)) = QUALITIES.iter().find(|(m, _)| *m == rel) {
        return format!("{name}{q}");
    }
    let triad = if rel & (1 << 3) != 0 && rel & (1 << 4) == 0 { "m" } else { "" };
    format!("{name}{triad}(+)")
}
