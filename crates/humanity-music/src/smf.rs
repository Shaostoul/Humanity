//! STANDARD MIDI FILES, in and out (docs/design/music-page.md sections 12 and 15a, rung 2): a piece as
//! NOTES IN TICKS ([`Notated`]), the shape a MIDI file holds and the one the MEI writer
//! (crate::mei_write) notates, read from and written to a Standard MIDI File through midly (the
//! Unlicense; the operator, 2026-11-20: "Yes. Let's download it.").
//!
//! Three ways in: a MIDI file ([`read_smf`]), a written piece as the MEI reader read it
//! ([`Notated::from_mei`], exact: its fractions of a whole note become ticks at 960 a quarter, which
//! holds triplets and quintuplets exactly), and a composed piece as the piano roll wrote it down
//! ([`Notated::from_sheet`]: each bar's seconds spread over its beats, so a tempo or a swing the
//! composer played lands where it was heard). One way out to MIDI ([`write_smf`]): a format 1 file, a
//! first track of the tempo, the meter, the key, the section letters as markers and the chord symbols
//! as text, then a track per part with its name, its program and its notes with their velocities.
//!
//! Nothing here runs on the audio thread: the Music page's export and import call it on the game
//! thread (a piece is kilobytes) or a test.

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::mei::{Frac, MeiScore};
use crate::scores::ScoreEntry;
use crate::sheet::Sheet;

/// Ticks a quarter note in every [`Notated`] (960 = 2^6 x 3 x 5: a triplet, a quintuplet and a
/// 64th all land on whole ticks).
pub const PPQ: u32 = 960;

/// The largest MIDI file the importer reads (a long orchestral file is about a megabyte).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// One note: where it starts and how long it lasts in ticks, its key (MIDI note number) and its
/// velocity (1 to 127).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PNote {
    pub tick: u64,
    pub len: u64,
    pub key: u8,
    pub vel: u8,
}

/// A part: a MIDI track (one channel of it) or a written staff.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NotatedPart {
    /// Its name as the file gives it (a track name, a staff's label; may be empty).
    pub name: String,
    /// The General MIDI program it asks for (0 to 127), when it names one.
    pub program: Option<u8>,
    /// Played on MIDI channel 10, the drums.
    pub drums: bool,
    /// Its role on the Music page when it came from one (an index into `tune::ROLE_IDS`).
    pub role: Option<u8>,
    pub notes: Vec<PNote>,
}

/// A piece in ticks: what a MIDI file holds, and what the MEI writer notates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notated {
    pub title: String,
    pub composer: String,
    /// Tempo changes: the tick and quarter notes a minute (the first at tick 0).
    pub tempos: Vec<(u64, f64)>,
    /// Meter changes: the tick, the count and the unit (3 and 8 for 3/8; the first at tick 0).
    pub meters: Vec<(u64, u8, u8)>,
    /// The key signature: sharps (positive) or flats (negative), and minor.
    pub key: Option<(i8, bool)>,
    /// Markers (the section letters) and text events (the chord symbols), each at its tick.
    pub markers: Vec<(u64, String)>,
    pub texts: Vec<(u64, String)>,
    pub parts: Vec<NotatedPart>,
}

impl Notated {
    /// The tick the last note ends on.
    pub fn end_tick(&self) -> u64 {
        self.parts.iter().flat_map(|p| p.notes.iter()).map(|n| n.tick + n.len).max().unwrap_or(0)
    }

    /// The quarter notes a minute at `tick`.
    pub fn qpm_at(&self, tick: u64) -> f64 {
        self.tempos.iter().rev().find(|t| t.0 <= tick).map(|t| t.1).unwrap_or(120.0)
    }

    /// Seconds from the start to `tick`, through the tempo changes.
    pub fn seconds_at(&self, tick: u64) -> f64 {
        let mut s = 0.0;
        let mut at = 0u64;
        let mut qpm = self.tempos.first().filter(|t| t.0 == 0).map(|t| t.1).unwrap_or(120.0);
        for &(t, q) in &self.tempos {
            if t >= tick {
                break;
            }
            if t > at {
                s += (t - at) as f64 / PPQ as f64 * 60.0 / qpm;
                at = t;
            }
            qpm = q;
        }
        s + (tick - at) as f64 / PPQ as f64 * 60.0 / qpm
    }

    /// A WRITTEN piece as the MEI reader read it (`score`, with its index entry for the default
    /// tempo, the key and each staff's name): every note at its exact place (a whole note is four
    /// quarters, so 3840 ticks), each staff a part, the tempo changes as written, the meter at each
    /// bar whose meter or length changes (a pickup bar is its own short meter, so the bar lines land
    /// where they were written), the key from the entry.
    pub fn from_mei(score: &MeiScore, entry: &ScoreEntry) -> Notated {
        let tick = |f: Frac| -> u64 { ((f.n as f64 / f.d as f64) * 4.0 * PPQ as f64).round().max(0.0) as u64 };
        let mut tempos: Vec<(u64, f64)> = score.tempos.iter().map(|t| (tick(t.at), t.qpm)).collect();
        if tempos.first().is_none_or(|t| t.0 > 0) {
            tempos.insert(0, (0, entry.qpm));
        }
        tempos.dedup_by(|b, a| b.0 == a.0 && {
            a.1 = b.1;
            true
        });
        let mut meters: Vec<(u64, u8, u8)> = Vec::new();
        for b in &score.bars {
            let (count, unit) = bar_meter(b.len, b.meter);
            if meters.last().is_none_or(|m| (m.1, m.2) != (count, unit)) {
                meters.push((tick(b.at), count, unit));
            }
        }
        if meters.is_empty() {
            meters.push((0, 4, 4));
        }
        let mut parts: Vec<NotatedPart> = Vec::new();
        for st in &score.staves {
            let spec = entry.staves.iter().find(|s| s.staff == st.n);
            let role = spec.and_then(|s| crate::tune::ROLE_IDS.iter().position(|r| *r == s.role)).map(|r| r as u8);
            let notes: Vec<PNote> = score
                .notes
                .iter()
                .filter(|n| n.staff == st.n && n.midi.is_finite())
                .map(|n| PNote { tick: tick(n.at), len: tick(n.dur).max(1), key: n.midi.round().clamp(0.0, 127.0) as u8, vel: (n.vel * 127.0).round().clamp(1.0, 127.0) as u8 })
                .collect();
            parts.push(NotatedPart { name: st.label.clone(), program: st.midi_program, drums: false, role, notes });
        }
        for p in &mut parts {
            p.notes.sort_by_key(|n| (n.tick, n.key));
        }
        Notated {
            title: if score.title.is_empty() { entry.name.clone() } else { score.title.clone() },
            composer: if score.composer.is_empty() { entry.composer.clone() } else { score.composer.clone() },
            tempos,
            meters,
            key: key_of(&entry.key_root, &entry.key_mode),
            markers: Vec::new(),
            texts: Vec::new(),
            parts,
        }
    }

    /// A COMPOSED piece as the piano roll wrote it down (`sheet`; `part_names`: each role's name for
    /// a person, in `tune::ROLE_IDS` order): each bar's seconds spread evenly over its beats (so the
    /// bar lines are the composer's own and a swung note lands where it was heard), a tempo change
    /// wherever a bar's length changes, the meter, the key of its first section, its section letters
    /// as markers and its chord symbols as text, and one part per role that plays.
    pub fn from_sheet(sheet: &Sheet, part_names: &[String], title: &str) -> Notated {
        let bpb = sheet.beats_per_bar.max(1) as u64;
        let bar_ticks = bpb * PPQ as u64;
        let mut bars = sheet.bars.clone();
        if bars.is_empty() {
            bars.push(0.0);
        }
        // Each bar's end (the last ends a bar's length on: the transcription stops mid-bar).
        let last_len = if bars.len() >= 2 { bars[bars.len() - 1] - bars[bars.len() - 2] } else { 2.0 };
        let end_of = |i: usize| -> f64 { bars.get(i + 1).copied().unwrap_or(bars[i] + last_len.max(0.1)) };
        let to_tick = |t: f64| -> u64 {
            let i = bars.iter().rposition(|&b| b <= t + 1e-9).unwrap_or(0);
            let (a, b) = (bars[i], end_of(i));
            let frac = ((t - a) / (b - a).max(1e-6)).max(0.0);
            (i as u64 * bar_ticks) as u64 + (frac * bar_ticks as f64).round() as u64
        };
        let mut tempos: Vec<(u64, f64)> = Vec::new();
        for i in 0..bars.len() {
            let qpm = bpb as f64 * 60.0 / (end_of(i) - bars[i]).max(1e-3);
            if tempos.last().is_none_or(|t: &(u64, f64)| (t.1 - qpm).abs() > 0.05) {
                tempos.push((i as u64 * bar_ticks, (qpm * 100.0).round() / 100.0));
            }
        }
        let mut parts: Vec<NotatedPart> = Vec::new();
        for role in 0..crate::tune::ROLES as u8 {
            let mut notes: Vec<PNote> = sheet
                .notes
                .iter()
                .filter(|n| crate::tune::part_of(n.role) == Some(role as usize) && n.midi.is_finite())
                .map(|n| {
                    let a = to_tick(n.at_s);
                    let b = to_tick(n.at_s + n.dur_s.max(0.0) as f64);
                    PNote { tick: a, len: b.saturating_sub(a).max(1), key: n.midi.round().clamp(0.0, 127.0) as u8, vel: (n.vel * 127.0).round().clamp(1.0, 127.0) as u8 }
                })
                .collect();
            if notes.is_empty() {
                continue;
            }
            notes.sort_by_key(|n| (n.tick, n.key));
            let name = part_names.get(role as usize).cloned().unwrap_or_else(|| crate::tune::ROLE_IDS[role as usize].to_string());
            parts.push(NotatedPart { name, program: None, drums: false, role: Some(role), notes });
        }
        let key = sheet.sections.first().map(|s| (fifths_of(s.key_pc, is_minor(s.mode)), is_minor(s.mode)));
        let flats = sheet.flats();
        Notated {
            title: title.to_string(),
            composer: String::new(),
            tempos,
            meters: vec![(0, bpb.min(255) as u8, 4)],
            key,
            markers: sheet.sections.iter().map(|s| (to_tick(s.at_s), ((b'A' + s.letter.min(25)) as char).to_string())).collect(),
            texts: sheet.chords.iter().map(|c| (to_tick(c.at_s), crate::sheet::chord_symbol(c.root, c.pcs, flats))).collect(),
            parts,
        }
    }
}

/// A bar's meter as a time signature: its written meter, or for a bar of another length (a pickup,
/// a short last bar) its length in the meter's unit (or in eighths, sixteenths ... until it is whole).
fn bar_meter(len: Frac, meter: (i64, i64)) -> (u8, u8) {
    let (count, unit) = (meter.0.clamp(1, 255), meter.1.clamp(1, 64));
    if len.n <= 0 || Frac::new(count, unit) == len {
        return (count as u8, unit as u8);
    }
    let mut u = unit.max(1);
    while u <= 64 {
        let c = len.n * u;
        if c % len.d == 0 && (1..=255).contains(&(c / len.d)) {
            return ((c / len.d) as u8, u as u8);
        }
        u *= 2;
    }
    (count as u8, unit as u8)
}

/// Whether a mode is heard as minor (its third is minor): what a key signature names it.
pub fn is_minor(mode: crate::tuning::Mode) -> bool {
    use crate::tuning::Mode::*;
    matches!(mode, Aeolian | Dorian | Phrygian | Locrian | MinorPentatonic | HarmonicMinor)
}

/// Sharps (positive) or flats (negative) of the key on `pc` (0 = C), major or minor.
pub fn fifths_of(pc: i32, minor: bool) -> i8 {
    // The major key with the same signature (a minor key's relative major: a minor third up).
    let major = (pc + if minor { 3 } else { 0 }).rem_euclid(12);
    // C G D A E B F# / Db Ab Eb Bb F: the flat side for the five black-key majors but F# (6 sharps).
    const FIFTHS: [i8; 12] = [0, -5, 2, -3, 4, -1, 6, 1, -4, 3, -2, 5];
    FIFTHS[major as usize]
}

/// The key signature of an index entry's `key_root` ("D", "A", "C sharp", "Bb") and `key_mode`.
pub fn key_of(root: &str, mode: &str) -> Option<(i8, bool)> {
    let r = root.trim();
    let mut chars = r.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str().trim().to_ascii_lowercase();
    let alter = if rest.starts_with('#') || rest.starts_with("sharp") {
        1
    } else if rest.starts_with('b') || rest.starts_with("flat") {
        -1
    } else {
        0
    };
    let minor = mode.trim().eq_ignore_ascii_case("minor");
    Some((fifths_of(base + alter, minor), minor))
}

/// Read a Standard MIDI File. Refused, in words: a file that is not one, a file timed in SMPTE
/// frames (not beats), and a file with no notes. Format 0 and 1 files give a part per track and
/// channel; format 2 (sequential songs) is refused, since its tracks are separate pieces.
pub fn read_smf(bytes: &[u8]) -> Result<Notated, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("the file is {} MB; the importer reads MIDI files up to {} MB", bytes.len() / (1024 * 1024), MAX_BYTES / (1024 * 1024)));
    }
    if bytes.len() < 14 || (&bytes[..4] != b"MThd" && &bytes[..4] != b"RIFF") {
        return Err("it is not a MIDI file: a Standard MIDI File begins with \"MThd\"".into());
    }
    let smf = Smf::parse(bytes).map_err(|e| format!("it could not be read as a MIDI file: {e}"))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u64,
        Timing::Timecode(..) => return Err("it is timed in film frames (SMPTE), not in beats; the importer reads files timed in beats".into()),
    };
    if ppq == 0 {
        return Err("it gives zero ticks a beat, so its notes have no length".into());
    }
    if smf.header.format == Format::Sequential {
        return Err("it is a format 2 file (several separate songs in one file); the importer reads format 0 and 1".into());
    }
    let scale = |t: u64| -> u64 { (t as u128 * PPQ as u128 / ppq as u128) as u64 };
    let mut n = Notated::default();
    // Per (track, channel): the part's index in `n.parts`.
    let mut part_of: std::collections::HashMap<(usize, u8), usize> = std::collections::HashMap::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0u64;
        let mut name = String::new();
        // Notes sounding: (channel, key) -> (start tick, velocity), a stack each (a key struck again
        // before it was let go ends the first).
        let mut on: std::collections::HashMap<(u8, u8), Vec<(u64, u8)>> = std::collections::HashMap::new();
        let mut programs: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(m) => match m {
                    MetaMessage::TrackName(b) if name.is_empty() => name = text_of(b),
                    MetaMessage::Tempo(us) if us.as_int() > 0 => n.tempos.push((scale(tick), 60_000_000.0 / us.as_int() as f64)),
                    MetaMessage::TimeSignature(num, den, _, _) if num > 0 && den <= 6 => n.meters.push((scale(tick), num, 1u8 << den)),
                    MetaMessage::KeySignature(sf, minor) if n.key.is_none() && (-7..=7).contains(&sf) => n.key = Some((sf, minor)),
                    MetaMessage::Marker(b) => n.markers.push((scale(tick), text_of(b))),
                    MetaMessage::Text(b) if !text_of(b).is_empty() => n.texts.push((scale(tick), text_of(b))),
                    _ => {}
                },
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    match message {
                        MidiMessage::ProgramChange { program } => {
                            programs.entry(ch).or_insert(program.as_int());
                        }
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => on.entry((ch, key.as_int())).or_default().push((tick, vel.as_int())),
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            if let Some(stack) = on.get_mut(&(ch, key.as_int())) {
                                if !stack.is_empty() {
                                    let (start, vel) = stack.remove(0);
                                    let idx = *part_of.entry((ti, ch)).or_insert_with(|| {
                                        n.parts.push(NotatedPart { drums: ch == 9, ..Default::default() });
                                        n.parts.len() - 1
                                    });
                                    let (a, b) = (scale(start), scale(tick));
                                    n.parts[idx].notes.push(PNote { tick: a, len: b.saturating_sub(a).max(1), key: key.as_int(), vel: vel.max(1) });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // Notes never let go end where the track ends.
        for ((ch, key), stack) in on {
            for (start, vel) in stack {
                let idx = *part_of.entry((ti, ch)).or_insert_with(|| {
                    n.parts.push(NotatedPart { drums: ch == 9, ..Default::default() });
                    n.parts.len() - 1
                });
                let (a, b) = (scale(start), scale(tick.max(start + ppq)));
                n.parts[idx].notes.push(PNote { tick: a, len: b.saturating_sub(a).max(1), key, vel: vel.max(1) });
            }
        }
        for (&(t, ch), &idx) in &part_of {
            if t == ti {
                n.parts[idx].name = name.clone();
                n.parts[idx].program = programs.get(&ch).copied();
            }
        }
        if ti == 0 && n.title.is_empty() && smf.tracks.len() > 1 {
            n.title = name.clone();
        }
    }
    n.parts.retain(|p| !p.notes.is_empty());
    if n.parts.is_empty() {
        return Err("it holds no notes".into());
    }
    for p in &mut n.parts {
        p.notes.sort_by_key(|x| (x.tick, x.key));
    }
    n.tempos.sort_by_key(|t| t.0);
    n.tempos.dedup_by(|b, a| b.0 == a.0 && {
        a.1 = b.1;
        true
    });
    if n.tempos.first().is_none_or(|t| t.0 > 0) {
        n.tempos.insert(0, (0, 120.0));
    }
    n.meters.sort_by_key(|m| m.0);
    n.meters.dedup_by(|b, a| b.0 == a.0 && {
        (a.1, a.2) = (b.1, b.2);
        true
    });
    if n.meters.first().is_none_or(|m| m.0 > 0) {
        n.meters.insert(0, (0, 4, 4));
    }
    n.markers.sort_by_key(|m| m.0);
    n.texts.sort_by_key(|m| m.0);
    Ok(n)
}

/// Text in a MIDI file (meant to be ASCII; Latin-1 in older files): read leniently.
fn text_of(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.trim().to_string(),
        Err(_) => b.iter().map(|&c| c as char).collect::<String>().trim().to_string(),
    }
}

/// Write `n` as a format 1 Standard MIDI File at 960 ticks a quarter: the first track the tempo
/// changes, the meters, the key, the title, the markers and the text events; then one track per part
/// (on its own channel, the drums on channel 10, skipping 10 for the rest) with its name, its
/// program and its notes.
pub fn write_smf(n: &Notated) -> Vec<u8> {
    let mut tracks: Vec<Vec<TrackEvent<'_>>> = Vec::new();
    // Every event carries its absolute tick until the deltas are worked out.
    let mut first: Vec<(u64, u8, TrackEventKind<'_>)> = Vec::new();
    if !n.title.is_empty() {
        first.push((0, 0, TrackEventKind::Meta(MetaMessage::TrackName(n.title.as_bytes()))));
    }
    for &(t, qpm) in &n.tempos {
        let us = (60_000_000.0 / qpm.max(1.0)).round().clamp(1.0, 16_777_215.0) as u32;
        first.push((t, 1, TrackEventKind::Meta(MetaMessage::Tempo(u24::new(us)))));
    }
    for &(t, count, unit) in &n.meters {
        let den = (unit.max(1) as f32).log2().round().clamp(0.0, 6.0) as u8;
        first.push((t, 1, TrackEventKind::Meta(MetaMessage::TimeSignature(count.max(1), den, 24, 8))));
    }
    if let Some((sf, minor)) = n.key {
        first.push((0, 1, TrackEventKind::Meta(MetaMessage::KeySignature(sf.clamp(-7, 7), minor))));
    }
    for (t, m) in &n.markers {
        first.push((*t, 2, TrackEventKind::Meta(MetaMessage::Marker(m.as_bytes()))));
    }
    for (t, m) in &n.texts {
        first.push((*t, 2, TrackEventKind::Meta(MetaMessage::Text(m.as_bytes()))));
    }
    tracks.push(with_deltas(first, n.end_tick()));
    let mut next_ch = 0u8;
    for p in &n.parts {
        let ch = if p.drums {
            9
        } else {
            let c = next_ch;
            next_ch = if next_ch == 8 { 10 } else { (next_ch + 1) % 16 };
            c
        };
        let channel = u4::new(ch);
        let mut evs: Vec<(u64, u8, TrackEventKind<'_>)> = Vec::new();
        if !p.name.is_empty() {
            evs.push((0, 0, TrackEventKind::Meta(MetaMessage::TrackName(p.name.as_bytes()))));
        }
        if let Some(prog) = p.program.filter(|_| !p.drums) {
            evs.push((0, 1, TrackEventKind::Midi { channel, message: MidiMessage::ProgramChange { program: u7::new(prog.min(127)) } }));
        }
        for note in &p.notes {
            let key = u7::new(note.key.min(127));
            // A note's end sorts before a note's start at the same tick, so a repeated key sounds again.
            evs.push((note.tick + note.len.max(1), 2, TrackEventKind::Midi { channel, message: MidiMessage::NoteOff { key, vel: u7::new(64) } }));
            evs.push((note.tick, 3, TrackEventKind::Midi { channel, message: MidiMessage::NoteOn { key, vel: u7::new(note.vel.clamp(1, 127)) } }));
        }
        tracks.push(with_deltas(evs, 0));
    }
    let smf = Smf { header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ as u16))), tracks };
    let mut out = Vec::new();
    // Writing into memory cannot fail.
    let _ = smf.write_std(&mut out);
    out
}

/// Events at absolute ticks (with a sort order among events of one tick) as a track: sorted, each
/// carrying the ticks since the one before, ended at `end` or the last event.
fn with_deltas<'a>(mut evs: Vec<(u64, u8, TrackEventKind<'a>)>, end: u64) -> Vec<TrackEvent<'a>> {
    evs.sort_by_key(|e| (e.0, e.1));
    let mut out = Vec::with_capacity(evs.len() + 1);
    let mut at = 0u64;
    for (t, _, kind) in evs {
        out.push(TrackEvent { delta: u28::new((t - at).min(0x0FFF_FFFF) as u32), kind });
        at = t;
    }
    out.push(TrackEvent { delta: u28::new(end.saturating_sub(at).min(0x0FFF_FFFF) as u32), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    out
}
