//! The score player's RUNG 2 (docs/design/score-player.md section 16): repeats and endings, grace
//! notes, octave lines, fermatas, hairpins and accents, articulations, and the written piece in the
//! piano roll. Each on a small MEI snippet against values worked out by hand, and measured in what
//! it produces (the notes' times and pitches, the seconds the timeline gives, what the player sends).
//! Each was seen failing first; the red is in its comment.

use std::sync::Arc;

use crate::mei::Frac;
use crate::sheet::Sheet;
use crate::tests::Recorder;
use crate::tests_written::{bar, mei, q};
use crate::written::{StaffVoice, Timeline, WrittenPlayer};

/// A bar numbered `n` holding one whole note of `pitch` in octave 4, with the measure's `attrs`.
fn whole(n: u32, pitch: &str, attrs: &str) -> String {
    format!(r#"<measure n="{n}"{attrs}><staff n="1"><layer n="1"><note pname="{pitch}" oct="4" dur="1"/></layer></staff></measure>"#)
}

/// Each note's (MIDI, start, length).
fn notes(s: &crate::mei::MeiScore) -> Vec<(f32, Frac, Frac)> {
    s.notes.iter().map(|n| (n.midi, n.at, n.dur)).collect()
}

/// REPEATS AND ENDINGS PLAY IN THE ORDER WRITTEN: `|: C D [1. E :|] [2. F] G` plays C D E C D F G (7
/// bars of the 5 written, the first ending only the first time, the second only the second), and a
/// repeat sign with no start sign goes back to the beginning. RED 2026-11-20 with the bars read once
/// in order: "left: [60.0, 62.0, 64.0, 65.0, 67.0], right: [60.0, 62.0, 64.0, 60.0, 62.0, 65.0,
/// 67.0]".
#[test]
fn repeats_and_endings_play_in_the_order_written() {
    let body = format!(
        r#"{}{}<ending n="1">{}</ending><ending n="2">{}</ending>{}"#,
        whole(1, "c", r#" left="rptstart""#),
        whole(2, "d", ""),
        whole(3, "e", r#" right="rptend""#),
        whole(4, "f", ""),
        whole(5, "g", "")
    );
    let s = mei(&body);
    let played: Vec<f32> = s.notes.iter().map(|n| n.midi).collect();
    assert_eq!(played, vec![60.0, 62.0, 64.0, 60.0, 62.0, 65.0, 67.0]);
    assert!(s.problems.is_empty(), "{:?}", s.problems);
    assert_eq!((s.bars.len(), s.written_bars, s.end), (7, 5, q(7, 1)));
    assert_eq!(s.notes.iter().map(|n| n.bar).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5, 6], "each note in the bar it is played in");
    // No start sign: back to the beginning, once.
    let s = mei(&format!("{}{}{}", whole(1, "c", ""), whole(2, "d", r#" right="rptend""#), whole(3, "e", "")));
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![60.0, 62.0, 60.0, 62.0, 64.0]);
    // Two repeats in a row (`rptboth` between them): each its own.
    let s = mei(&format!("{}{}{}", whole(1, "c", r#" left="rptstart" right="rptboth""#), whole(2, "d", r#" right="rptend""#), whole(3, "e", "")));
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![60.0, 60.0, 62.0, 62.0, 64.0]);
}

/// GRACE NOTES TAKE THEIR TIME FROM THE NOTE AFTER THEM: a slashed grace note (`grace="unacc"`)
/// before a quarter C sounds on the beat for a 32nd and the C follows it (7/32 left); an appoggiatura
/// (`grace="acc"`) before a half takes half of it; a group of three before a quarter, in a
/// `graceGrp`, a 32nd each. RED 2026-11-20 with grace notes counted and not played: "left: [(60.0,
/// Frac { n: 0, d: 1 }, Frac { n: 1, d: 4 }), (60.0, Frac { n: 1, d: 4 }, Frac { n: 1, d: 2 })],
/// right: [(59.0, Frac { n: 0, d: 1 }, Frac { n: 1, d: 32 }), ...]".
#[test]
fn grace_notes_take_their_time_from_the_note_after_them() {
    let s = mei(&bar(r#"<note pname="b" oct="3" dur="8" grace="unacc"/><note pname="c" oct="4" dur="4"/><note pname="d" oct="4" dur="8" grace="acc"/><note pname="c" oct="4" dur="2"/><rest dur="4"/>"#));
    assert_eq!(notes(&s), vec![(59.0, q(0, 1), q(1, 32)), (60.0, q(1, 32), q(7, 32)), (62.0, q(1, 4), q(1, 4)), (60.0, q(1, 2), q(1, 4))]);
    assert_eq!(s.notes.iter().map(|n| n.grace).collect::<Vec<_>>(), vec![true, false, true, false]);
    assert!(s.not_played.is_empty() && s.problems.is_empty(), "{:?} {:?}", s.not_played, s.problems);
    let s = mei(&bar(r#"<graceGrp grace="unacc"><beam><note pname="g" oct="4" dur="16"/><note pname="a" oct="4" dur="16"/><note pname="b" oct="4" dur="16"/></beam></graceGrp><note pname="c" oct="5" dur="4"/><rest dur="2" dots="1"/>"#));
    assert_eq!(notes(&s), vec![(67.0, q(0, 1), q(1, 32)), (69.0, q(1, 32), q(1, 32)), (71.0, q(1, 16), q(1, 32)), (72.0, q(3, 32), q(5, 32))]);
    assert!(s.not_played.is_empty(), "{:?}", s.not_played);
}

/// AN OCTAVE LINE MOVES THE NOTES UNDER IT: `dis="8"` above from the first note to the second sounds
/// them an octave up (84, 86) and not the note after (76); `dis="15"` below from beat 2 to the next
/// bar's first beat (`tstamp2="1m+1"`) two octaves down, and a note with its own `oct.ges` keeps it.
/// RED 2026-11-20 with octave lines a problem, not applied: "left: [72.0, 74.0, 76.0], right: [84.0,
/// 86.0, 76.0]".
#[test]
fn an_octave_line_moves_the_notes_under_it() {
    let s = mei(r##"<measure n="1"><staff n="1"><layer n="1"><note xml:id="o1" pname="c" oct="5" dur="4"/><note xml:id="o2" pname="d" oct="5" dur="4"/><note pname="e" oct="5" dur="2"/></layer></staff><octave staff="1" dis="8" dis.place="above" startid="#o1" endid="#o2"/></measure>"##);
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![84.0, 86.0, 76.0]);
    assert!(s.problems.is_empty(), "{:?}", s.problems);
    let four = r#"<note pname="c" oct="4" dur="4"/><note pname="c" oct="4" dur="4"/><note pname="c" oct="4" dur="4" oct.ges="4"/><note pname="c" oct="4" dur="4"/>"#;
    let s = mei(&format!(
        r#"<measure n="1"><staff n="1"><layer n="1">{four}</layer></staff><octave staff="1" dis="15" dis.place="below" tstamp="2" tstamp2="1m+1"/></measure><measure n="2"><staff n="1"><layer n="1">{four}</layer></staff></measure>"#
    ));
    assert_eq!(s.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), vec![60.0, 36.0, 60.0, 36.0, 36.0, 60.0, 60.0, 60.0]);
    assert!(s.problems.is_empty(), "{:?}", s.problems);
}

/// A FERMATA HOLDS ITS NOTE: at 60 quarter notes a minute, C D E F with a fermata on D, the D lasts 2
/// s (twice its written second), E starts at 3 s and the bar ends at 5 s; a fermata written as the
/// note's attribute holds the same, and fermatas on every staff at one moment hold it once. RED
/// 2026-11-20 with fermatas counted and not played: "left: [], right: [(Frac { n: 1, d: 4 }, Frac {
/// n: 1, d: 4 })]".
#[test]
fn a_fermata_holds_its_note() {
    let layer = |attr: &str| format!(r#"<note pname="c" oct="4" dur="4"/><note xml:id="f2" pname="d" oct="4" dur="4"{attr}/><note pname="e" oct="4" dur="4"/><note pname="f" oct="4" dur="4"/>"#);
    for (layer, control) in [(layer(""), r##"<fermata staff="1" startid="#f2"/><fermata staff="1" tstamp="2"/>"##), (layer(r#" fermata="above""#), "")] {
        let s = mei(&format!(r#"<measure n="1"><staff n="1"><layer n="1">{layer}</layer></staff>{control}</measure>"#));
        assert_eq!(s.fermatas, vec![(q(1, 4), q(1, 4))]);
        let t = Timeline::from_mei(&s, 60.0);
        assert_eq!(t.notes.iter().map(|n| n.at_s).collect::<Vec<_>>(), vec![0.0, 1.0, 3.0, 4.0]);
        assert_eq!((t.notes[1].dur_s, t.length_s), (2.0, 5.0));
        assert!(s.not_played.is_empty(), "{:?}", s.not_played);
    }
}

/// A HAIRPIN MOVES THE LOUDNESS AND AN ACCENT MARKS ONE NOTE: piano, then a crescendo to the forte at
/// the next bar's first note, raises the four quarters by their place (0.42, 0.515, 0.61, 0.705) to
/// 0.8; a diminuendo with no dynamic at its end lowers the level one step (0.12) by its end and the
/// level holds after it; `sf` makes its one note louder and the next is back at the level. RED
/// 2026-11-20 with hairpins counted and not played: "left: [0.42, 0.42, 0.42, 0.42, 0.8], right:
/// [0.42, 0.515, 0.61, 0.705, 0.8]".
#[test]
fn a_hairpin_moves_the_loudness_and_an_accent_marks_one_note() {
    let quarters = |p: &str| (1..=4).map(|k| format!(r#"<note xml:id="{p}{k}" pname="c" oct="4" dur="4"/>"#)).collect::<String>();
    let s = mei(&format!(
        r##"<measure n="1"><staff n="1"><layer n="1">{}</layer></staff><dynam staff="1" tstamp="1">p</dynam><hairpin form="cres" staff="1" startid="#h1" endid="#n1"/></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="1"/></layer></staff><dynam staff="1" tstamp="1">f</dynam></measure>"##,
        quarters("h")
    ));
    let vel: Vec<f32> = s.notes.iter().map(|n| (n.vel * 1000.0).round() / 1000.0).collect();
    assert_eq!(vel, vec![0.42, 0.515, 0.61, 0.705, 0.8]);
    let s = mei(&format!(
        r##"<measure n="1"><staff n="1"><layer n="1">{}</layer></staff><dynam staff="1" tstamp="1">mf</dynam><hairpin form="dim" staff="1" startid="#d1" endid="#d4"/></measure><measure n="2"><staff n="1"><layer n="1">{}</layer></staff><dynam staff="1" tstamp="2">sf</dynam></measure>"##,
        quarters("d"),
        quarters("e")
    ));
    let vel: Vec<f32> = s.notes.iter().map(|n| (n.vel * 1000.0).round() / 1000.0).collect();
    assert_eq!(vel, vec![0.68, 0.64, 0.6, 0.56, 0.56, 0.81, 0.56, 0.56]);
    assert!(s.not_played.is_empty() && s.problems.is_empty(), "{:?} {:?}", s.not_played, s.problems);
    // "cresc." as words ending on a rest in a later bar (the Mountain King's \cresc runs to the rest
    // its \ff is on): the rest closes it. RED 2026-11-20 with a span's end id met only on a note:
    // "problems: [\"a line from 0.000 whole notes waits for Id(\\\"w5\\\"), which never came: applied to
    // the end\"]".
    let s = mei(&format!(
        r##"<measure n="1"><staff n="1"><layer n="1">{}</layer></staff><dynam staff="1" tstamp="1">p</dynam><dynam staff="1" startid="#w1" endid="#w5">cresc.</dynam></measure><measure n="2"><staff n="1"><layer n="1"><rest xml:id="w5" dur="4"/><note pname="c" oct="4" dur="2" dots="1"/></layer></staff><dynam staff="1" tstamp="1">f</dynam></measure>"##,
        quarters("w")
    ));
    assert!(s.problems.is_empty(), "problems: {:?}", s.problems);
    let vel: Vec<f32> = s.notes.iter().map(|n| (n.vel * 1000.0).round() / 1000.0).collect();
    assert_eq!(vel, vec![0.42, 0.515, 0.61, 0.705, 0.8]);
}

/// ARTICULATIONS SHAPE HOW A NOTE IS PLAYED, and the player sends what the timeline holds: at 120 a
/// staccato quarter (0.5 s) is held 0.25 s, a tenuto all but 5 ms, a plain one a tenth short (0.45
/// s); an accent (as a child `artic`) is struck 0.12 harder. RED 2026-11-20 with articulations
/// counted and not played: "left: [0.45, 0.45, 0.45, 0.45], right: [0.25, 0.495, 0.45, 0.45]".
#[test]
fn articulations_shape_how_a_note_is_played() {
    let s = mei(&bar(r#"<note pname="c" oct="4" dur="4" artic="stacc"/><note pname="c" oct="4" dur="4" artic="ten"/><note pname="c" oct="4" dur="4"><artic artic="acc"/></note><note pname="c" oct="4" dur="4"/>"#));
    let t = Timeline::from_mei(&s, 120.0);
    let holds: Vec<f32> = t.notes.iter().map(|n| (n.hold_s * 1000.0).round() / 1000.0).collect();
    assert_eq!(holds, vec![0.25, 0.495, 0.45, 0.45]);
    let vel: Vec<f32> = t.notes.iter().map(|n| (n.vel * 1000.0).round() / 1000.0).collect();
    assert_eq!(vel, vec![0.68, 0.68, 0.8, 0.68]);
    assert!(s.not_played.is_empty(), "{:?}", s.not_played);
    // What the player sends: the staccato note let go after 0.25 s.
    let mut p = WrittenPlayer::new(Arc::new(t), vec![StaffVoice { staff: 1, inst: 0, role: 5, level: 1.0, pan: 0.0 }], 48000.0, 432.0, 0.0, 0.0);
    let mut rec = Recorder::default();
    p.process(&mut rec);
    assert_eq!(rec.notes[0].1.hold_s, 0.25);
}

/// A MEASURED TREMOLO PLAYS ITS STROKES: a half note C3 with eighth strokes (`bTrem unitdur="8"`, the
/// symphony's string and timpani rolls) plays four eighth notes, and a chord so marked four chords;
/// the first keeps the note's id (a tie into it still joins). RED 2026-11-20 with bTrem counted and
/// its note played once: "left: [(48.0, Frac { n: 0, d: 1 }, Frac { n: 1, d: 2 }), (52.0, Frac {
/// n: 1, d: 2 }, Frac { n: 1, d: 4 }), ...], right: [(48.0, Frac { n: 0, d: 1 }, Frac { n: 1, d: 8 }),
/// ...]".
#[test]
fn a_measured_tremolo_plays_its_strokes() {
    let s = mei(&bar(r#"<bTrem unitdur="8"><note xml:id="t1" pname="c" oct="3" dur="2"/></bTrem><bTrem unitdur="16"><chord dur="4"><note pname="e" oct="3"/><note pname="g" oct="3"/></chord></bTrem><rest dur="4"/>"#));
    let e = q(1, 8);
    let s16 = q(1, 16);
    assert_eq!(
        notes(&s),
        vec![
            (48.0, q(0, 1), e),
            (48.0, q(1, 8), e),
            (48.0, q(2, 8), e),
            (48.0, q(3, 8), e),
            (52.0, q(1, 2), s16),
            (55.0, q(1, 2), s16),
            (52.0, q(9, 16), s16),
            (55.0, q(9, 16), s16),
            (52.0, q(10, 16), s16),
            (55.0, q(10, 16), s16),
            (52.0, q(11, 16), s16),
            (55.0, q(11, 16), s16),
        ]
    );
    assert_eq!(s.notes[0].id, "t1");
    assert!(s.not_played.is_empty(), "{:?}", s.not_played);
}

/// THE PIANO ROLL DRAWS A WRITTEN PIECE: the sheet made from a timeline holds every note at its time
/// and length in its staff's role, a bar line at each bar's start, the bar's beats, and the piece's
/// length; the roll's notes for the first bar start on its beats. RED 2026-11-20 with the stub that
/// made an empty sheet: "left: 0, right: 5".
#[test]
fn the_piano_roll_draws_a_written_piece() {
    let s = mei(&format!(
        "{}{}",
        bar(r#"<note pname="c" oct="4" dur="4"/><note pname="d" oct="4" dur="4"/><note pname="e" oct="4" dur="4"/><note pname="f" oct="4" dur="4"/>"#),
        bar(r#"<note pname="g" oct="4" dur="1"/>"#)
    ));
    let t = Timeline::from_mei(&s, 120.0);
    let sheet = Sheet::from_timeline(&t, &[(1, 5)]);
    assert_eq!(sheet.notes.len(), 5);
    assert_eq!(sheet.notes.iter().map(|n| (n.at_s, n.midi, n.role)).collect::<Vec<_>>(), vec![(0.0, 60.0, 5), (0.5, 62.0, 5), (1.0, 64.0, 5), (1.5, 65.0, 5), (2.0, 67.0, 5)]);
    assert_eq!((sheet.bars.clone(), sheet.beats_per_bar, sheet.seconds), (vec![0.0, 2.0], 4, 4.0));
    let roll: Vec<f32> = sheet.bar_notes(0).iter().map(|n| n.start_beat).collect();
    assert_eq!(roll, vec![0.0, 1.0, 2.0, 3.0]);
    // A staff the index gives no role is drawn in no role's colour (255) and left out of the roll.
    assert_eq!(Sheet::from_timeline(&t, &[]).notes[0].role, 255);
}
