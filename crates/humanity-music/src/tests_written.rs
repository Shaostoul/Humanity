//! The score player's tests (docs/design/score-player.md; crate::mei, crate::written, crate::scores):
//! the MEI reader on small MEI snippets, one element at a time, against values worked out by hand
//! (a dotted quarter is 3/8 of a whole note, F in D major is F sharp, MIDI 66); the timeline's
//! seconds; the player through the real score, measured in what it PRODUCES (the sample a note
//! sounds from in the rendered buffer, the pitch counted in the sound, the loudness by the app's one
//! meter); and the shipped piece read with nothing skipped. Each was seen failing first; the red is in
//! its comment.

use std::sync::Arc;

use humanity_core::audio::levels::LoudnessTargets;
use humanity_core::audio::loudness::LoudnessMeter;

use crate::mei::{read, Frac, MeiScore, WordPos};
use crate::resolved::Resolved;
use crate::score::{MusicContext, ScoreEngine};
use crate::scores::{load_index, load_piece};
use crate::tests::{data_dir, shipped, Recorder};
use crate::written::{StaffVoice, Timeline, WrittenPlayer};
use crate::SampleLibrary;

const RATE: f32 = 48000.0;

/// An MEI file around `measures`, with `def` as the score's definition attributes and one staff.
pub(crate) fn mei_in(def: &str, staves: &str, measures: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.1+basic">
  <meiHead><fileDesc><titleStmt><title>Test</title></titleStmt><pubStmt/></fileDesc></meiHead>
  <music><body><mdiv><score>
    <scoreDef {def}><staffGrp>{staves}</staffGrp></scoreDef>
    <section>{measures}</section>
  </score></mdiv></body></music>
</mei>"#
    )
}

/// One staff in C major, 4/4, around `measures`.
pub(crate) fn mei(measures: &str) -> MeiScore {
    read(&mei_in(r#"keysig="0" meter.count="4" meter.unit="4""#, r#"<staffDef n="1" lines="5"/>"#, measures)).unwrap_or_else(|e| panic!("{e}"))
}

/// One bar of one layer.
pub(crate) fn bar(layer: &str) -> String {
    format!(r#"<measure n="1"><staff n="1"><layer n="1">{layer}</layer></staff></measure>"#)
}

pub(crate) fn q(n: i64, d: i64) -> Frac {
    Frac::new(n, d)
}

/// A NOTE READS ITS PITCH AND ITS DURATION: C4 a quarter is MIDI 60 lasting 1/4 of a whole note, A4 a
/// half is 69 for 1/2, starting where the first ends. RED 2026-11-20 with the octave counted from C0
/// as 0 instead of MIDI's -1 (`12 * oct`): "left: (48.0, Frac { n: 0, d: 1 }, ...), right: (60.0,
/// ...)".
#[test]
fn a_note_reads_its_pitch_and_its_duration() {
    let s = mei(&bar(r#"<note xml:id="a" pname="c" oct="4" dur="4"/><note xml:id="b" pname="a" oct="4" dur="2"/>"#));
    assert_eq!(s.notes.len(), 2);
    assert_eq!((s.notes[0].midi, s.notes[0].at, s.notes[0].dur, s.notes[0].id.as_str()), (60.0, q(0, 1), q(1, 4), "a"));
    assert_eq!((s.notes[1].midi, s.notes[1].at, s.notes[1].dur), (69.0, q(1, 4), q(1, 2)));
    assert!(s.problems.is_empty() && s.skipped.is_empty(), "{:?} {:?}", s.problems, s.skipped);
}

/// DOTS LENGTHEN A NOTE BY HALF, THEN BY A QUARTER: a dotted quarter is 3/8, a double-dotted quarter
/// 7/16, and `dot` elements count as `dots` does. RED 2026-11-20 with the dots ignored: "left: Frac {
/// n: 1, d: 4 }, right: Frac { n: 3, d: 8 }".
#[test]
fn dots_lengthen_a_note_by_half_then_a_quarter() {
    let s = mei(&bar(r#"<note pname="c" oct="4" dur="4" dots="1"/><note pname="d" oct="4" dur="4" dots="2"/><note pname="e" oct="4" dur="8"><dot/></note>"#));
    assert_eq!(s.notes[0].dur, q(3, 8));
    assert_eq!(s.notes[1].at, q(3, 8));
    assert_eq!(s.notes[1].dur, q(7, 16));
    assert_eq!(s.notes[2].dur, q(3, 16));
}

/// RESTS, SPACES AND WHOLE-BAR RESTS TAKE TIME: an eighth rest puts the note after it at 1/8, a space
/// counts like a rest, and a whole-bar rest in 3/4 is a bar of 3/4 (the next bar starts there). RED
/// 2026-11-20 with the dots ignored (the dotted half after the bar's rest): "left: Frac { n: 5, d: 4
/// }, right: Frac { n: 3, d: 2 }".
#[test]
fn rests_spaces_and_whole_bar_rests_take_time() {
    let s = mei(&bar(r#"<rest dur="8"/><note pname="c" oct="4" dur="8"/><space dur="4"/><note pname="d" oct="4" dur="2"/>"#));
    assert_eq!(s.notes[0].at, q(1, 8));
    assert_eq!(s.notes[1].at, q(1, 2));
    let s = read(&mei_in(
        r#"meter.count="3" meter.unit="4""#,
        r#"<staffDef n="1" lines="5"/>"#,
        r#"<measure n="1"><staff n="1"><layer n="1"><mRest/></layer></staff></measure><measure n="2"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="2" dots="1"/></layer></staff></measure>"#,
    ))
    .unwrap();
    assert_eq!(s.bars[0].len, q(3, 4));
    assert_eq!(s.notes[0].at, q(3, 4));
    assert_eq!(s.end, q(3, 2));
}

/// A CHORD'S NOTES SOUND TOGETHER, for the chord's length, and time moves on once. RED 2026-11-20 with
/// the octave counted from C0: "left: [48.0, 52.0, 55.0, 60.0], right: [60.0, 64.0, 67.0, 72.0]".
#[test]
fn a_chords_notes_sound_together() {
    let s = mei(&bar(r#"<chord dur="2"><note pname="c" oct="4"/><note pname="e" oct="4"/><note pname="g" oct="4"/></chord><note pname="c" oct="5" dur="2"/>"#));
    assert_eq!(s.notes.len(), 4);
    for n in &s.notes[..3] {
        assert_eq!((n.at, n.dur), (q(0, 1), q(1, 2)));
    }
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![60.0, 64.0, 67.0, 72.0]);
    assert_eq!(s.notes[3].at, q(1, 2));
}

/// A TRIPLET'S NOTES SHARE THE BEAT EXACTLY: three eighths in the time of two are 1/12 each and the
/// note after them starts on the second beat (1/4), no rounding; a triplet inside a triplet
/// multiplies. RED 2026-11-20 with `numbase/num` turned over (`num/numbase`): "left: Frac { n: 3, d:
/// 16 }, right: Frac { n: 1, d: 12 }".
#[test]
fn a_triplets_notes_share_the_beat_exactly() {
    let s = mei(&bar(r#"<tuplet num="3" numbase="2"><note pname="c" oct="4" dur="8"/><note pname="d" oct="4" dur="8"/><note pname="e" oct="4" dur="8"/></tuplet><note pname="f" oct="4" dur="4"/>"#));
    assert_eq!(s.notes[0].dur, q(1, 12));
    assert_eq!(s.notes[2].at, q(1, 6));
    assert_eq!(s.notes[3].at, q(1, 4));
    let s = mei(&bar(r#"<tuplet num="3" numbase="2"><tuplet num="3" numbase="2"><note pname="c" oct="4" dur="16"/><note pname="c" oct="4" dur="16"/><note pname="c" oct="4" dur="16"/></tuplet><note pname="d" oct="4" dur="8"/><note pname="e" oct="4" dur="8"/></tuplet><note pname="f" oct="4" dur="4"/>"#));
    assert_eq!(s.notes[0].dur, q(1, 36));
    assert_eq!(s.notes[5].at, q(1, 4), "{:?}", s.notes.iter().map(|n| n.at).collect::<Vec<_>>());
}

/// THE KEY SIGNATURE AND THE BAR'S ACCIDENTALS SET THE PITCH (MEI Basic has no `accid.ges`, so the
/// reader works it out as a musician reads it): in D major (`keysig="2s"`) a written F sounds F sharp
/// (66) and C sounds C sharp; a natural holds to the end of its bar and not into the next; in E flat
/// (`3f`) B, E and A are flat. The full-MEI spellings (`keySig sig`, `key.sig`) read the same. RED
/// 2026-11-20 with the key ignored: "F in D major: left: 65.0, right: 66.0".
#[test]
fn the_key_signature_and_the_bars_accidentals_set_the_pitch() {
    let staves = r#"<staffDef n="1" lines="5"/>"#;
    let m = r#"<measure n="1"><staff n="1"><layer n="1"><note pname="f" oct="4" dur="4"/><note pname="c" oct="5" dur="4"/><note pname="f" oct="4" dur="4" accid="n"/><note pname="f" oct="4" dur="4"/></layer></staff></measure><measure n="2"><staff n="1"><layer n="1"><note pname="f" oct="4" dur="1"/></layer></staff></measure>"#;
    let s = read(&mei_in(r#"keysig="2s" meter.count="4" meter.unit="4""#, staves, m)).unwrap();
    let p: Vec<f32> = s.notes.iter().map(|n| n.midi).collect();
    assert_eq!(p[0], 66.0, "F in D major");
    assert_eq!(p[1], 73.0, "C in D major");
    assert_eq!(p[2], 65.0, "a natural");
    assert_eq!(p[3], 65.0, "the natural holds to the end of the bar");
    assert_eq!(p[4], 66.0, "and not into the next");
    let s = read(&mei_in(r#"keysig="3f""#, staves, &bar(r#"<note pname="b" oct="4" dur="4"/><note pname="e" oct="4" dur="4"/><note pname="a" oct="4" dur="4"/><note pname="d" oct="4" dur="4"/>"#))).unwrap();
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![70.0, 63.0, 68.0, 62.0]);
    let s = read(&mei_in("", r#"<staffDef n="1" lines="5"><keySig sig="1s"/></staffDef>"#, &bar(r#"<note pname="f" oct="4" dur="1"/>"#))).unwrap();
    assert_eq!(s.notes[0].midi, 66.0, "keySig@sig on a staffDef");
    let s = read(&mei_in(r#"key.sig="1f""#, staves, &bar(r#"<note pname="b" oct="4" dur="4"/><note pname="b" oct="4" dur="4"><accid accid="s"/></note><note pname="b" oct="4" dur="2" accid.ges="n"/>"#))).unwrap();
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![70.0, 72.0, 71.0], "key.sig, a child accid, accid.ges");
}

/// QUARTER-TONE ACCIDENTALS ARE HALF A SEMITONE: A4 with `1qs` is MIDI 69.5 (the voice plays
/// `note_hz(69.5)`), with `3qf` 67.5. RED 2026-11-20 with the octave counted from C0: "left: (57.5,
/// 55.5), right: (69.5, 67.5)".
#[test]
fn quarter_tone_accidentals_are_half_a_semitone() {
    let s = mei(&bar(r#"<note pname="a" oct="4" dur="2" accid="1qs"/><note pname="a" oct="4" dur="2" accid="3qf"/>"#));
    assert_eq!((s.notes[0].midi, s.notes[1].midi), (69.5, 67.5));
}

/// A TIE JOINS TWO NOTES INTO ONE: `tie="i"` and `tie="t"` make one C lasting 3/4; a `tie` element
/// across the bar line (MuseScore writes ties so) joins too, and the tied note keeps the sharp written
/// before the bar line while the next F in the new bar is natural again. RED 2026-11-20 with ties not
/// joined: "left: 2, right: 1".
#[test]
fn a_tie_joins_two_notes_into_one() {
    let s = mei(&bar(r#"<note pname="c" oct="4" dur="2" tie="i"/><note pname="c" oct="4" dur="4" tie="t"/><rest dur="4"/>"#));
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.notes[0].dur, q(3, 4));
    let m = r##"<measure n="1"><staff n="1"><layer n="1"><rest dur="2"/><note xml:id="t1" pname="f" oct="4" dur="2" accid="s"/></layer></staff><tie startid="#t1" endid="#t2"/></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="t2" pname="f" oct="4" dur="4"/><note pname="f" oct="4" dur="2" dots="1"/></layer></staff></measure>"##;
    let s = mei(m);
    assert_eq!(s.notes.len(), 2, "{:?}", s.notes);
    assert_eq!((s.notes[0].midi, s.notes[0].at, s.notes[0].dur), (66.0, q(1, 2), q(3, 4)));
    assert_eq!(s.notes[1].midi, 65.0, "the sharp does not carry past the tie");
    assert!(s.problems.is_empty(), "{:?}", s.problems);
}

/// THE LAYERS OF A STAFF PLAY TOGETHER, each from the bar's start, and the bar lasts as long as its
/// longest layer (so a pickup bar is as short as it is written). RED 2026-11-20 with a second layer
/// starting where the first ended: "left: 1, right: 2".
#[test]
fn layers_play_together_and_the_longest_sets_the_bar() {
    let s = mei(r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="5" dur="1"/></layer><layer n="2"><note pname="c" oct="4" dur="4"/><note pname="d" oct="4" dur="4"/><note pname="e" oct="4" dur="2"/></layer></staff></measure>"#);
    assert_eq!(s.notes.len(), 4);
    assert_eq!(s.notes[0].at, q(0, 1));
    assert_eq!(s.notes.iter().filter(|n| n.at == q(0, 1)).count(), 2);
    let s = mei(r#"<measure n="0"><staff n="1"><layer n="1"><note pname="g" oct="4" dur="4"/></layer></staff></measure><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="5" dur="1"/></layer></staff></measure>"#);
    assert_eq!(s.bars[0].len, q(1, 4), "a pickup bar");
    assert_eq!(s.notes[1].at, q(1, 4));
}

/// STAVES KEEP THEIR NUMBERS AND NAMES, and a transposing staff sounds where it should: a clarinet in
/// B flat (`trans.semi="-2"`) written D5 sounds C5 (72). RED 2026-11-20 with the octave counted from
/// C0: "left: [(1, 60.0), (2, 60.0)], right: [(1, 72.0), (2, 72.0)]".
#[test]
fn staves_keep_their_numbers_names_and_transposition() {
    let staves = r#"<staffDef n="1" lines="5" label="Flute"/><staffDef n="2" lines="5" trans.semi="-2"><label>Clarinet in B flat</label><instrDef midi.instrnum="71"/></staffDef>"#;
    let m = r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="5" dur="1"/></layer></staff><staff n="2"><layer n="1"><note pname="d" oct="5" dur="1"/></layer></staff></measure>"#;
    let s = read(&mei_in("", staves, m)).unwrap();
    assert_eq!(s.staves.iter().map(|x| (x.n, x.label.as_str(), x.midi_program)).collect::<Vec<_>>(), vec![(1, "Flute", None), (2, "Clarinet in B flat", Some(71))]);
    assert_eq!(s.notes.iter().map(|n| (n.staff, n.midi)).collect::<Vec<_>>(), vec![(1, 72.0), (2, 72.0)]);
}

/// A TEMPO MARK READS ITS NUMBER AND ITS PLACE: `midi.bpm` 90 at `tstamp` 3 (the third beat) starts at
/// 1/2; `mm` 80 of half notes is 160 quarter notes a minute; a dotted quarter unit at 60 is 90. RED
/// 2026-11-20 with `mm` read as quarter notes whatever its unit: "left: [..., (Frac { n: 1, d: 1 },
/// 80.0), (Frac { n: 2, d: 1 }, 60.0)], right: [..., 160.0), ..., 90.0)]".
#[test]
fn a_tempo_mark_reads_its_number_and_its_place() {
    let s = mei(r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="1"/></layer></staff><tempo tstamp="3" staff="1" midi.bpm="90"/></measure><measure n="2"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="1"/></layer></staff><tempo tstamp="1" staff="1" mm="80" mm.unit="2">Allegro</tempo></measure><measure n="3"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="1"/></layer></staff><tempo tstamp="1" staff="1" mm="60" mm.unit="4" mm.dots="1"/><tempo tstamp="2" staff="1">a tempo</tempo></measure>"#);
    assert_eq!(s.tempos.iter().map(|t| (t.at, t.qpm)).collect::<Vec<_>>(), vec![(q(1, 2), 90.0), (q(1, 1), 160.0), (q(2, 1), 90.0)]);
    assert_eq!(s.not_played.get("tempo (words without a number)"), Some(&1));
}

/// LYRICS KEEP THEIR NOTE IDS, WORD POSITIONS AND VERSES (the sing-along's hook): two verses under the
/// same notes, `wordpos` and `con`, a `syl` attribute, a melisma's second note with no syllable, and a
/// syllable written under a tie's second note belonging to the sounding (first) note. RED 2026-11-20
/// with each syllable keeping the index its note had before the tie was joined (the reader then
/// crashed sorting them): "index out of bounds: the len is 4 but the index is 4"; and with ties not
/// joined at all the held syllable sat on a fifth note: "(\"e\", 4, 1, \"held\", ...)".
#[test]
fn lyrics_keep_their_note_ids_word_positions_and_verses() {
    let s = mei(&bar(
        r#"<note xml:id="a" pname="c" oct="4" dur="4"><verse n="1"><syl wordpos="i" con="d">Freu</syl></verse><verse n="2"><syl>Joy</syl></verse></note><note xml:id="b" pname="d" oct="4" dur="8"><verse n="1"><syl wordpos="t" con="u">de</syl></verse></note><note xml:id="c" pname="e" oct="4" dur="8"/><note xml:id="d" pname="f" oct="4" dur="4" syl="la" tie="i"/><note xml:id="e" pname="f" oct="4" dur="4" tie="t"><verse><syl>held</syl></verse></note>"#,
    ));
    let got: Vec<(&str, usize, u16, &str, WordPos, Option<char>)> = s.syllables.iter().map(|x| (x.note_id.as_str(), x.note, x.verse, x.text.as_str(), x.wordpos, x.con)).collect();
    assert_eq!(
        got,
        vec![
            ("a", 0, 1, "Freu", WordPos::Initial, Some('d')),
            ("a", 0, 2, "Joy", WordPos::Single, None),
            ("b", 1, 1, "de", WordPos::Terminal, Some('u')),
            ("d", 3, 1, "la", WordPos::Single, None),
            ("e", 3, 1, "held", WordPos::Single, None),
        ]
    );
    assert_eq!(s.notes.len(), 4, "the tie joined two of five notes");
}

/// DYNAMICS AND VELOCITY SET HOW HARD A NOTE IS STRUCK: piano marked on the third beat, where the
/// note's own `vel` (100 of 127) wins, so the note before it is still mezzo-forte (the default); forte
/// from the next bar (0.8). RED 2026-11-20 with the dynamics never applied: "left: (0.68, 0.68),
/// right: (0.8, 0.8)".
#[test]
fn dynamics_and_velocity_set_how_hard_a_note_is_struck() {
    let s = mei(r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="2"/><note pname="d" oct="4" dur="2" vel="100"/></layer></staff><dynam tstamp="3" staff="1">p</dynam></measure><measure n="2"><staff n="1"><layer n="1"><note pname="e" oct="4" dur="2"/><note pname="f" oct="4" dur="2"/></layer></staff><dynam tstamp="1" staff="1">f</dynam></measure>"#);
    let v: Vec<f32> = s.notes.iter().map(|n| n.vel).collect();
    assert_eq!(v[0], crate::mei::DEFAULT_VEL, "before any mark");
    assert!((v[1] - 100.0 / 127.0).abs() < 1e-6, "its own vel");
    assert_eq!((v[2], v[3]), (0.8, 0.8));
}

/// AN ELEMENT THE READER DOES NOT KNOW IS SKIPPED AND COUNTED, NEVER A CRASH: an unknown element (with
/// a note inside, not read), a slur and a trill counted as known but not played, a measure repeat
/// (`mRpt`) and a `tupletSpan` listed as problems (they would change what sounds), and broken files
/// give errors that name the line. RED 2026-11-20 with unknown elements walked into (their note read):
/// "left: 3, right: 2". (Rung 2, 2026-11-20: grace notes, octave lines and repeat signs, which this
/// test held as not played or as problems in rung 1, are played now: tests_written2.rs.)
#[test]
fn unknown_elements_are_skipped_and_counted_never_a_crash() {
    let s = mei(r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="2"/><wobble><note pname="d" oct="4" dur="2"/></wobble><mRpt/><note pname="e" oct="4" dur="2"/></layer></staff><slur startid="x" endid="y"/><trill tstamp="1"/><tupletSpan tstamp="1" tstamp2="1m+1"/></measure>"#);
    assert_eq!(s.notes.len(), 2);
    assert_eq!(s.skipped.get("wobble"), Some(&1));
    assert_eq!(s.not_played.get("slur"), Some(&1));
    assert_eq!(s.not_played.get("trill"), Some(&1));
    assert_eq!(s.problems.len(), 2, "{:?}", s.problems);
    assert!(s.problems.iter().any(|p| p.contains("mRpt")) && s.problems.iter().any(|p| p.contains("tupletSpan")));
    let broken = read("<mei><music><body>\n<mdiv>\n</body></music></mei>").unwrap_err();
    assert!(broken.contains("line"), "{broken}");
    assert!(read("<score/>").unwrap_err().contains("not an MEI file"));
    assert!(read("<mei><meiHead/></mei>").unwrap_err().contains("no <music>"));
    assert!(read("").is_err());
    assert!(read("<mei><music>&nbsp;</music></mei>").is_err(), "an entity XML does not define");
}

/// THE TEMPO MAP TURNS BEATS INTO SECONDS across a change: four quarters at 60 take four seconds, and
/// after a change to 120 at the second bar each quarter takes half a second. RED 2026-11-20 with every
/// segment timed at the first tempo: "left: [0.0, 4.0, 5.0, 6.0], right: [0.0, 4.0, 4.5, 5.0]".
#[test]
fn the_tempo_map_turns_beats_into_seconds_across_a_change() {
    let s = mei(r#"<measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="1"/></layer></staff><tempo tstamp="1" staff="1" midi.bpm="60"/></measure><measure n="2"><staff n="1"><layer n="1"><note pname="d" oct="4" dur="4"/><note pname="e" oct="4" dur="4"/><note pname="f" oct="4" dur="2"/></layer></staff><tempo tstamp="1" staff="1" midi.bpm="120"/></measure>"#);
    let t = Timeline::from_mei(&s, 100.0);
    let at: Vec<f64> = t.notes.iter().map(|n| n.at_s).collect();
    assert_eq!(at, vec![0.0, 4.0, 4.5, 5.0]);
    assert_eq!(t.notes[0].dur_s, 4.0);
    assert_eq!(t.length_s, 6.0);
    assert_eq!(t.bars.iter().map(|b| (b.at_s, b.qpm)).collect::<Vec<_>>(), vec![(0.0, 60.0), (4.0, 120.0)]);
}

/// The shipped library resolved with no recordings.
fn res() -> Resolved {
    Resolved::new(&shipped(), &SampleLibrary::new()).unwrap()
}

/// A score playing `timeline` on the clarinet (A = `a4`).
fn written_score(timeline: Timeline, a4: f32) -> ScoreEngine {
    let lib = shipped();
    let r = res();
    let voices = vec![StaffVoice { staff: 1, inst: r.instrument_index("clarinet").unwrap(), role: 5, level: 1.0, pan: 0.0 }];
    let player = WrittenPlayer::new(Arc::new(timeline), voices, RATE, a4, 4.0, 0.0);
    let ctx = MusicContext { a4_hz: a4, mood: lib.mood_index("mothership_calm").unwrap() as u16, ..Default::default() };
    ScoreEngine::written(r, RATE, 1, ctx, player)
}

/// A DOTTED QUARTER AT 72 LANDS AT 1,250 MS, to the sample: after a dotted-quarter rest at 72 quarter
/// notes a minute (1.5 beats x 5/6 s) the note starts at 1.250 s, sample 60,000, which is NOT on the
/// score's 64-sample chunk grid (937.5 chunks), so the RENDERED buffer must be silent before sample
/// 60,000 and sounding by sample 60,008; and the player sends that note at sample 60,000 exactly. (A
/// first version at 90 landed on sample 48,000 = 750 chunks, where cutting time in whole chunks
/// could not be told from cutting it at the note: it could not fail.) RED 2026-11-20 with the score
/// cutting time in whole chunks whatever the next note: "the first sound at sample 60032".
#[test]
fn a_dotted_quarter_at_72_lands_at_1250_ms() {
    let s = mei(&bar(r#"<rest dur="4" dots="1"/><note pname="a" oct="4" dur="8"/><rest dur="2"/>"#).replace("</measure>", r#"<tempo tstamp="1" staff="1" midi.bpm="72"/></measure>"#));
    let t = Timeline::from_mei(&s, 120.0);
    assert_eq!(t.notes[0].at_s, 1.25);
    let mut score = written_score(t.clone(), 432.0);
    let mut buf = vec![[0f32; 2]; 64_000];
    score.render(&mut buf);
    let first = buf.iter().position(|f| f[0] != 0.0 || f[1] != 0.0).expect("the note sounds");
    assert!((60_000..=60_008).contains(&first), "the first sound at sample {first}");
    // The player's own output: the note sent at sample 60,000.
    let mut p = WrittenPlayer::new(Arc::new(t), vec![StaffVoice { staff: 1, inst: 0, role: 5, level: 1.0, pan: 0.0 }], RATE, 432.0, 0.0, 0.0);
    let mut rec = Recorder::default();
    let mut pos = 0u64;
    while rec.notes.is_empty() && pos < 100_000 {
        rec.now_s = pos as f64;
        p.process(&mut rec);
        let n = p.until_next();
        p.advance(n);
        pos += n as u64;
    }
    assert_eq!(rec.notes[0].0, 60_000.0);
    // An eighth at 72 lasts 5/12 s; it is held a tenth short (under the 50 ms most).
    assert!((rec.notes[0].1.hold_s - (5.0 / 12.0 - 5.0 / 120.0) as f32).abs() < 1e-4, "an eighth at 72 held a tenth short: {}", rec.notes[0].1.hold_s);
}

/// The shipped score's file.
fn shipped_mei() -> MeiScore {
    let text = std::fs::read_to_string(data_dir().join("music").join("scores").join("ode_to_joy.mei")).unwrap();
    read(&text).unwrap_or_else(|e| panic!("{e}"))
}

/// THE SHIPPED SCORE PARSES WITH ZERO SKIPPED CORE ELEMENTS: the Ode to Joy file reads with nothing
/// skipped, nothing left unplayed and no problem; 16 bars of 4/4 (16 whole notes), 62 notes, 60
/// syllables each on the note whose id it names, the key of D (every F and C sharp: no MIDI 65 or
/// 72), its title and composer from the header, and the index lists it. RED 2026-11-20 with `<beam>`
/// skipped instead of read through: "skipped: {\"beam\": 2}".
#[test]
fn the_shipped_score_parses_with_nothing_skipped() {
    let s = shipped_mei();
    assert!(s.skipped.is_empty(), "skipped: {:?}", s.skipped);
    assert!(s.not_played.is_empty(), "not played: {:?}", s.not_played);
    assert!(s.problems.is_empty(), "problems: {:?}", s.problems);
    assert_eq!(s.meiversion, "5.1+basic");
    assert_eq!((s.bars.len(), s.end, s.notes.len(), s.syllables.len()), (16, q(16, 1), 62, 60));
    for x in &s.syllables {
        assert_eq!(s.notes[x.note].id, x.note_id, "{:?}", x.text);
    }
    assert!(s.notes.iter().all(|n| n.midi != 65.0 && n.midi != 72.0), "D major has no F or C natural");
    assert_eq!(s.notes[0].midi, 66.0, "it begins on F sharp");
    assert_eq!(s.title, "Ode to Joy (the theme, a simple version)");
    assert_eq!(s.composer, "Ludwig van Beethoven");
    assert!(s.availability.contains("CC0"));
    let words: Vec<&str> = s.syllables.iter().take(4).map(|x| x.text.as_str()).collect();
    assert_eq!(words, vec!["Freu", "de,", "schö", "ner"]);
    let lib = shipped();
    let entry = lib.score("ode_to_joy").expect("the index lists it");
    let piece = load_piece(&data_dir(), entry).unwrap();
    assert_eq!(piece.timeline.length_s, 32.0, "16 bars of 4 at 120");
}

/// THE SING-ALONG'S HOOK FINDS THE SYLLABLE SUNG NOW: in the shipped piece at 120, "Freu" from 0 s,
/// "de," from 0.5 s, nothing before the start, and the melisma's second note keeps "den" sounding.
/// RED 2026-11-20 with the first syllable returned instead of the last begun: "left: Some(\"Freu\"),
/// right: Some(\"de,\")".
#[test]
fn syllable_at_finds_the_syllable_sung_now() {
    let t = Timeline::from_mei(&shipped_mei(), 120.0);
    assert_eq!(t.syllable_at(-0.1, 1), None);
    assert_eq!(t.syllable_at(0.1, 1).map(|s| s.text.as_str()), Some("Freu"));
    assert_eq!(t.syllable_at(0.6, 1).map(|s| s.text.as_str()), Some("de,"));
    let den = t.syllables.iter().find(|s| s.text == "den").unwrap();
    let after_g = den.at_s + 0.3;
    assert_eq!(t.syllable_at(after_g, 1).map(|s| s.text.as_str()), Some("den"));
    assert_eq!(t.syllable_at(0.1, 2), None, "no second verse");
}

/// A WRITTEN PIECE PLAYS THROUGH, RESTS, THEN PLAYS AGAIN: two times through the shipped piece send
/// its 62 notes twice, the second time from 36 s (32 s and the 4 s rest), and the status counts its
/// bars. RED 2026-11-20 with the player not going back to its first note after the rest: "62 notes".
#[test]
fn a_written_piece_plays_through_rests_then_plays_again() {
    let lib = shipped();
    let piece = load_piece(&data_dir(), lib.score("ode_to_joy").unwrap()).unwrap();
    let mut p = piece.player(&res(), RATE, 432.0);
    let mut rec = Recorder::default();
    let end = (72.0 * RATE) as u64;
    let mut pos = 0u64;
    while pos < end {
        rec.now_s = pos as f64 / RATE as f64;
        p.process(&mut rec);
        let n = p.until_next().min((end - pos) as usize);
        p.advance(n);
        pos += n as u64;
    }
    assert_eq!(rec.notes.len(), 124, "{} notes", rec.notes.len());
    assert_eq!(rec.notes[62].0, 36.0);
    assert_eq!(p.loops(), 1);
    assert_eq!(rec.notes[0].1.a4, 432.0);
}

/// A WRITTEN PIECE PLAYS AT THE TUNING: its first note, F sharp 4, counted in the rendered sound is
/// 363.3 Hz at A = 432 (432 x 2^(-3/12)) and 370.0 Hz at 440, within 0.5 Hz. RED 2026-11-20 with the
/// player sending a fixed 440 as the note's A4: "at 432: 370.37 Hz, want 363.27".
#[test]
fn a_written_piece_plays_at_the_tuning() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let piece = load_piece(&data_dir(), lib.score("ode_to_joy").unwrap()).unwrap();
    for a4 in [432.0f32, 440.0] {
        let mut score = ScoreEngine::written(res(), RATE, 1, MusicContext { a4_hz: a4, ..Default::default() }, piece.player(&res(), RATE, a4));
        let mut buf = vec![[0f32; 2]; (0.45 * RATE) as usize];
        score.render(&mut buf);
        let x: Vec<f32> = buf[(0.1 * RATE) as usize..].iter().map(|f| f[0] + f[1]).collect();
        let want = crate::tuning::note_hz(66.0, a4) as f64;
        let mut best = (0.0, 0.0);
        let mut f = want - 20.0;
        while f <= want + 20.0 {
            let pw = goertzel(&x, f);
            if pw > best.1 {
                best = (f, pw);
            }
            f += 0.05;
        }
        assert!((best.0 - want).abs() < 0.5, "at {a4}: {:.2} Hz, want {want:.2}", best.0);
    }
}

fn goertzel(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let cw = 2.0 * (2.0 * std::f64::consts::PI * hz / RATE as f64).cos();
    let (mut q1, mut q2) = (0.0, 0.0);
    for (i, &v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
        let q0 = cw * q1 - q2 + v as f64 * w;
        q2 = q1;
        q1 = q0;
    }
    q1 * q1 + q2 * q2 - cw * q1 * q2
}

/// THE SHIPPED PIECES SOUND AT THE MUSIC TARGET UNDER THE TRUE-PEAK CEILING (data/audio/loudness.toml,
/// by the app's one BS.1770 meter): each piece's `level_db` in the index puts one time through within
/// 1.5 LU of the music target, under the ceiling. RED 2026-11-20 with the index's level at 0 dB (in a
/// run that also had two of this file's other gates broken, on pitch only): "ode_to_joy: -31.2 LUFS,
/// the target -18.0"; the level was then set from it. Rung 2 (2026-11-20) measures every piece before
/// the verdict; the three converted pieces at 0 dB read "fur_elise: -37.1 LUFS", "mountain_king: -26.7
/// LUFS" and "beethoven_symphony5_1: -15.5 LUFS", and their levels were set from that (19.1, 8.7 and
/// -2.5 dB). Rendered a second at a time.
#[test]
fn the_shipped_pieces_sound_at_the_music_target() {
    let _heavy = crate::tests_audio::heavy();
    let targets = LoudnessTargets::load(&data_dir()).unwrap();
    let music = targets.target_lufs("music");
    let lib = shipped();
    let mut off: Vec<String> = Vec::new();
    for entry in &lib.scores {
        let piece = load_piece(&data_dir(), entry).unwrap();
        let ctx = MusicContext { mood: lib.mood_index("mothership_calm").unwrap() as u16, ..Default::default() };
        let mut score = ScoreEngine::written(res(), RATE, 1, ctx, piece.player(&res(), RATE, 432.0));
        // A second at a time (the symphony's six minutes held whole would be 140 MB), metered from
        // 6 s: the score's own fade-in (its 1.5 s and 4 s smoothers) is not the piece's level.
        let total = ((piece.timeline.length_s + 2.0) * RATE as f64) as usize;
        let mut buf = vec![[0f32; 2]; RATE as usize];
        let mut m = LoudnessMeter::new(RATE as u32, 2);
        let mut done = 0usize;
        while done < total {
            let n = buf.len().min(total - done);
            score.render(&mut buf[..n]);
            for (k, f) in buf[..n].iter().enumerate() {
                if done + k >= (6.0 * RATE) as usize {
                    m.push_frame(f);
                }
            }
            done += n;
        }
        m.finish();
        let lufs = m.integrated().unwrap_or(f64::NEG_INFINITY);
        // Every piece measured before the verdict, so one run gives each level to set.
        if (lufs - music).abs() > 1.5 {
            off.push(format!("{}: {lufs:.1} LUFS, the target {music:.1}", entry.id));
        }
        if m.true_peak_dbtp() > targets.ceiling_dbtp {
            off.push(format!("{}: true peak {:.2} dBTP", entry.id, m.true_peak_dbtp()));
        }
    }
    assert!(off.is_empty(), "{off:?}");
}

/// A WRITTEN PIECE'S `render` ALLOCATES NOTHING (the crate's rule): two times through the shipped piece
/// with the tuning changed on the way, counted by tests_audio's counting allocator on this thread.
/// RED 2026-11-20 with the player cloning its voice list for each note: "render allocated 258 times".
#[test]
fn a_written_pieces_render_allocates_nothing() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let piece = load_piece(&data_dir(), lib.score("ode_to_joy").unwrap()).unwrap();
    let mut score = ScoreEngine::written(res(), RATE, 1, MusicContext::default(), piece.player(&res(), RATE, 432.0));
    let mut buf = vec![[0f32; 2]; 128];
    for _ in 0..200 {
        score.render(&mut buf);
    }
    crate::tests_audio::counting::COUNT.with(|c| c.set(0));
    crate::tests_audio::counting::ON.with(|o| o.set(true));
    let blocks = (75.0 * RATE / 128.0) as usize;
    for k in 0..blocks {
        if k == blocks / 2 {
            score.set_context(MusicContext { a4_hz: 440.0, ..Default::default() });
        }
        score.render(&mut buf);
    }
    crate::tests_audio::counting::ON.with(|o| o.set(false));
    let n = crate::tests_audio::counting::COUNT.with(|c| c.get());
    assert_eq!(n, 0, "render allocated {n} times");
    assert!(score.status().bars > 16, "two times through: {} bars", score.status().bars);
}

/// THE INDEX REFUSES WHAT IT CANNOT PLAY: a repeated id, a file outside the scores folder, a role that
/// is not one of the Music page's, and a staff naming an instrument the library lacks (the library's
/// check); no index at all is no written pieces, not an error. RED 2026-11-20 with a repeated id let
/// through: "called `Result::unwrap_err()` on an `Ok` value: [ScoreEntry { id: \"a\", ...".
#[test]
fn the_index_refuses_what_it_cannot_play() {
    let dir = std::env::temp_dir().join(format!("hos-scores-test-{}", std::process::id()));
    let scores = dir.join("music").join("scores");
    std::fs::create_dir_all(&scores).unwrap();
    assert_eq!(load_index(&dir.join("nowhere")).unwrap(), Vec::new());
    let entry = |id: &str, file: &str, role: &str| format!(r#"(id: "{id}", file: "{file}", staves: [(staff: 1, instrument: "clarinet", role: "{role}")])"#);
    let cases = [
        (format!("Scores(pieces: [{}, {}])", entry("a", "a.mei", "melody"), entry("a", "b.mei", "melody")), "used twice"),
        (format!("Scores(pieces: [{}])", entry("a", "../a.mei", "melody")), "plain .mei"),
        (format!("Scores(pieces: [{}])", entry("a", "a.mei", "lead")), "role"),
    ];
    for (text, why) in cases {
        std::fs::write(scores.join("index.ron"), text).unwrap();
        let e = load_index(&dir).unwrap_err();
        assert!(e.contains(why), "{e}");
    }
    let mut lib = shipped();
    lib.scores[0].staves[0].instrument = "kazoo".into();
    assert!(lib.check().unwrap_err().contains("kazoo"));
    let _ = std::fs::remove_dir_all(&dir);
}
