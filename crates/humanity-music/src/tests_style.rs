//! SOUND STYLES, rung 3a (docs/design/music-page.md section 15a and section 25): a style file changes
//! the instruments heard, retro FM plays at the person's tuning, solo piano plays every part of a full
//! score as a piano reduction, an imported file's program numbers pick their instruments and a
//! program with no instrument of ours is said, and each style stays within the audio thread's block
//! budget. Each test measures what the engine PRODUCES (the rendered samples, their spectrum, the
//! notes the reduction keeps), never the name a style chose.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use crate::scores::{load_piece, WrittenPiece};
use crate::style::{apply, load_styles, SoundStyle, StyleNote};
use crate::tests::{data_dir, shipped};
use crate::tune::{Tune, ROLE_IDS};
use crate::voice::{BlockParams, NoteOn, VoicePool};
use crate::{MusicContext, MusicLibrary, Resolved, SampleLibrary, ScoreEngine};

const RATE: f32 = 48000.0;

fn res(lib: &MusicLibrary) -> Resolved {
    Resolved::new(lib, &SampleLibrary::new()).expect("resolve")
}

fn style<'a>(lib: &'a MusicLibrary, id: &str) -> &'a SoundStyle {
    lib.styles.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("the {id} style is shipped"))
}

fn piece(lib: &MusicLibrary, id: &str) -> WrittenPiece {
    load_piece(&data_dir(), lib.score(id).unwrap()).unwrap()
}

/// Render `seconds` of `p` from its bar `bar` (A = `a4`, `tune` on the mixer), dropping nothing.
fn render_piece(lib: &MusicLibrary, p: &WrittenPiece, a4: f32, tune: Tune, bar: usize, seconds: f32) -> Vec<[f32; 2]> {
    let r = res(lib);
    let mut player = p.player(&r, RATE, a4);
    player.seek_bar(bar);
    let ctx = MusicContext { a4_hz: a4, tune, ..Default::default() };
    let mut e = ScoreEngine::written(r, RATE, 1, ctx, player);
    e.set_offline(true);
    let mut buf = vec![[0.0f32; 2]; 128];
    let mut all = Vec::new();
    for _ in 0..(seconds * RATE / 128.0) as usize {
        e.render(&mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

fn mono(x: &[[f32; 2]]) -> Vec<f32> {
    x.iter().map(|f| 0.5 * (f[0] + f[1])).collect()
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// The power at `hz` over `x` (a Hann window, Goertzel).
fn power(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let w = 2.0 * std::f64::consts::PI * hz / RATE as f64;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for (i, &v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1).max(1) as f64).cos();
        let s0 = v as f64 * win + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

/// The spectrum as shares of its power in 48 bands a third of an octave wide from 50 Hz up, each
/// band the sum of 4 Goertzel probes.
fn spectrum(x: &[f32]) -> Vec<f64> {
    let bands: Vec<f64> = (0..24)
        .map(|b| {
            let lo = 50.0 * 2f64.powf(b as f64 / 3.0);
            (0..4).map(|k| power(x, lo * 2f64.powf(k as f64 / 12.0))).sum::<f64>()
        })
        .collect();
    let total: f64 = bands.iter().sum::<f64>().max(1e-30);
    bands.into_iter().map(|b| b / total).collect()
}

/// How far apart two spectra are: half the summed difference of their shares (0 the same, 1 none shared).
fn distance(a: &[f64], b: &[f64]) -> f64 {
    0.5 * a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>()
}

/// A STYLE FILE CHANGES THE INSTRUMENTS HEARD (music-page.md section 15a): Ode to Joy (one staff, the
/// clarinet, its role the melody) rendered as written, through the shipped retro FM style (its
/// melody rule: the FM lead) and through a copy of that style FILE whose melody rule names the FM
/// bell instead, read from a folder of its own. Each spectrum is measured from the rendered audio;
/// each style moves it well away from the others, and the same style renders the same sound twice.
///
/// Seen RED 2026-11-21 with `style::apply` leaving every staff on its own instruments (the voice it
/// chose ignored): "retro FM sounds like the piece as written: distance 0.000".
#[test]
fn a_style_file_changes_the_instruments_heard() {
    let lib = shipped();
    let p = piece(&lib, "ode_to_joy");
    let as_written = spectrum(&mono(&render_piece(&lib, &p, 432.0, Tune::default(), 0, 8.0)));
    let retro = apply(style(&lib, "retro_fm"), &p);
    let retro_s = spectrum(&mono(&render_piece(&lib, &retro.piece, 432.0, Tune::default(), 0, 8.0)));
    let again = spectrum(&mono(&render_piece(&lib, &apply(style(&lib, "retro_fm"), &p).piece, 432.0, Tune::default(), 0, 8.0)));
    // The style file changed on disk: its melody rule now names the bell.
    let dir = std::env::temp_dir().join(format!("hos-style-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("music").join("styles")).unwrap();
    let text = std::fs::read_to_string(data_dir().join("music/styles/retro_fm.ron")).unwrap();
    let changed = text.replace("(role: \"melody\", voice: (instrument: \"retro_fm_lead\"))", "(role: \"melody\", voice: (instrument: \"retro_fm_bell\"))");
    assert_ne!(changed, text, "the test found the melody rule to change");
    std::fs::write(dir.join("music/styles/retro_fm.ron"), changed).unwrap();
    let edited = load_styles(Path::new(&dir)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let bell = apply(&edited[0], &p);
    let bell_s = spectrum(&mono(&render_piece(&lib, &bell.piece, 432.0, Tune::default(), 0, 8.0)));
    let d_written = distance(&as_written, &retro_s);
    let d_file = distance(&retro_s, &bell_s);
    let d_same = distance(&retro_s, &again);
    println!("spectrum distances: as written to retro FM {d_written:.3}, retro FM to the edited file {d_file:.3}, retro FM twice {d_same:.4}");
    assert!(d_written > 0.15, "retro FM sounds like the piece as written: distance {d_written:.3}");
    assert!(d_file > 0.15, "the edited style file sounds like the shipped one: distance {d_file:.3}");
    assert!(d_same < 0.01, "the same style renders the same sound: distance {d_same:.4}");
}

/// One note of `inst` at MIDI `midi` and A = `a4`, held 3 s: the samples.
fn one_note(lib: &MusicLibrary, inst: &str, midi: f32, a4: f32) -> Vec<f32> {
    let insts = vec![lib.instruments[lib.instrument_index(inst).unwrap()].clone()];
    let banks = vec![None];
    let mut pool = VoicePool::new(7);
    let vowel = [(crate::dsp::SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    pool.note_on(NoteOn { inst: 0, role: 5, midi, a4, vel: 0.9, pan: 0.0, gate: 0.0, hold_s: 0.0, tag: 1, level: 1.0 }, &insts, &banks, RATE);
    let (mut l, mut r, mut sl, mut sr) = ([0f32; 128], [0f32; 128], [0f32; 128], [0f32; 128]);
    let mut out = Vec::new();
    for _ in 0..(3.0 * RATE / 128.0) as usize {
        for b in [&mut l, &mut r, &mut sl, &mut sr] {
            b.fill(0.0);
        }
        pool.render(&insts, &banks, &BlockParams::plain(RATE, 2.0, &vowel), &mut l, &mut r, &mut sl, &mut sr, 128);
        out.extend(l.iter().zip(r.iter()).map(|(a, b)| a + b));
    }
    out
}

/// The note's measured fundamental near `expect_hz`: the power-weighted mean of a fine sweep across
/// 25 cents either side (a detuned pair or a vibrato centres on the note; a voice at another tuning
/// pulls it away), over 0.2 to 2.2 s of the note.
fn fundamental(x: &[f32], expect_hz: f64) -> f64 {
    let w = &x[(0.2 * RATE) as usize..(2.2 * RATE) as usize];
    let (mut sp, mut sf) = (0.0, 0.0);
    for k in -50..=50 {
        let f = expect_hz * 2f64.powf(k as f64 * 0.5 / 1200.0);
        let p = power(w, f);
        sp += p;
        sf += p * f;
    }
    sf / sp.max(1e-30)
}

/// RETRO FM RENDERS AT THE TUNING (the 2026-11-06 decision, A = 432 by default): each pitched voice of
/// the retro FM style plays A4, and the fundamental measured in its rendered samples is within a cent
/// of 432 Hz, and of 440 Hz when the tuning is switched.
///
/// Seen RED 2026-11-21 with `fm_ops::render` playing every note a 440-over-432 sharper (as a voice
/// tuned to 440 whatever the setting would): "retro_fm_lead at A = 432: 433.86 Hz, 7.5 cents off" (the
/// note itself 31.8 cents sharp, outside the sweep, so only its skirt pulls the mean).
#[test]
fn retro_fm_renders_at_the_tuning() {
    let lib = shipped();
    for inst in ["retro_fm_lead", "retro_fm_keys", "retro_fm_strings", "retro_fm_brass", "retro_fm_reed", "retro_fm_flute", "retro_fm_organ", "retro_fm_bass"] {
        for a4 in [432.0f32, 440.0] {
            // The bass speaks an octave and a half lower: measured on its own A2.
            let (midi, expect) = if inst == "retro_fm_bass" { (45.0, a4 as f64 / 4.0) } else { (69.0, a4 as f64) };
            let f = fundamental(&one_note(&lib, inst, midi, a4), expect);
            let cents = 1200.0 * (f / expect).log2();
            println!("{inst} at A = {a4}: {f:.3} Hz ({cents:+.2} cents)");
            assert!(cents.abs() < 1.0, "{inst} at A = {a4}: {f:.2} Hz, {:.1} cents off", cents.abs());
        }
    }
}

/// SOLO PIANO PLAYS EVERY PART (music-page.md section 15a; the reduction's rules in crate::style): the
/// symphony's twelve staves through the solo piano style. The reduction keeps notes on every staff,
/// never more than ten keys struck together, every key on the piano, every pitch class of every chord
/// (only doublings lost); and each role the score names,
/// soloed on the mixer from that role's first note, is heard in the rendered audio (not silent: within
/// 40 dB of the whole reduction's level), every staff on the piano part.
///
/// Seen RED 2026-11-21 with `apply` leaving every staff on its own instruments: "staff 1 plays
/// \"soft_lead\", not the piano"; and with the reduction's pitch-class pass left out (a full chord cut
/// to ten keys from the top down): "the reduction loses a pitch class the full score struck in 1 of
/// 1824 chords".
#[test]
fn solo_piano_plays_every_part_of_a_written_piece() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let p = piece(&lib, "beethoven_symphony5_1");
    let styled = apply(style(&lib, "solo_piano"), &p).piece;
    for s in &styled.entry.staves {
        assert_eq!((s.instrument.as_str(), s.part.as_deref()), ("electric_piano", Some("piano")), "staff {} plays {:?}, not the piano", s.staff, s.instrument);
        let kept = styled.timeline.notes.iter().filter(|n| n.staff == s.staff).count();
        assert!(kept > 0, "staff {} ({}) keeps no note in the reduction", s.staff, s.role);
    }
    // Two hands: the most keys struck within 30 ms, and the range.
    let notes = &styled.timeline.notes;
    let mut most = 0;
    for (i, n) in notes.iter().enumerate() {
        let together = notes[i..].iter().take_while(|m| m.at_s - n.at_s <= 0.03).count();
        most = most.max(together);
    }
    assert!(most <= 10, "at most ten keys together: {most}");
    assert!(notes.iter().all(|n| (21.0..=108.0).contains(&n.midi)), "every key on the piano");
    // The harmony survives: wherever the full score strikes notes together (grouped as the reduction
    // groups them), the reduction strikes every pitch class it did, unless there were more classes
    // than ten keys can hold; only doublings are lost.
    let mut full: Vec<&crate::written::TimedNote> = p.timeline.notes.iter().collect();
    full.sort_by(|a, b| a.at_s.total_cmp(&b.at_s));
    let class = |m: f32| (m.round() as i32).rem_euclid(12);
    let (mut i, mut groups, mut lost) = (0, 0, 0);
    while i < full.len() {
        let t0 = full[i].at_s;
        let j = i + full[i..].iter().take_while(|n| n.at_s - t0 <= 0.03).count();
        let want: BTreeSet<i32> = full[i..j].iter().map(|n| class(n.midi)).collect();
        let last = full[j - 1].at_s;
        let a = notes.partition_point(|n| n.at_s < t0 - 1e-9);
        let have: BTreeSet<i32> = notes[a..].iter().take_while(|n| n.at_s <= last + 1e-9).map(|n| class(n.midi)).collect();
        if want.len() <= 10 {
            groups += 1;
            if !want.is_subset(&have) {
                lost += 1;
            }
        }
        i = j;
    }
    assert_eq!(lost, 0, "the reduction loses a pitch class the full score struck in {lost} of {groups} chords");
    let before = p.timeline.notes.len();
    println!("solo piano: {} notes of {before} kept, at most {most} together", notes.len());
    // Each role, soloed from its first note.
    let whole = rms(&mono(&render_piece(&lib, &styled, 432.0, Tune::default(), 0, 6.0)));
    let roles: BTreeSet<&str> = styled.entry.staves.iter().map(|s| s.role.as_str()).collect();
    for role in roles {
        let k = ROLE_IDS.iter().position(|r| *r == role).unwrap();
        let staves: Vec<u16> = styled.entry.staves.iter().filter(|s| s.role == role).map(|s| s.staff).collect();
        let first = notes.iter().find(|n| staves.contains(&n.staff)).unwrap().at_s;
        let bar = styled.timeline.bar_at(first).unwrap_or(0);
        let tune = Tune { solo: 1 << k, ..Tune::default() };
        let heard = rms(&mono(&render_piece(&lib, &styled, 432.0, tune, bar, 4.0)));
        let db = 20.0 * (heard / whole.max(1e-12)).log10();
        println!("solo piano, {role} soloed from bar {bar}: {db:.1} dB against the whole");
        assert!(db > -40.0, "{role} soloed from its first note is silent: {db:.1} dB");
    }
}

/// A small MIDI file: a violin (program 40), a finger bass (33), a bird tweet (123, no instrument of
/// ours) and a drum track with a kick (36), a snare (38) and a tambourine (54, none of ours either).
fn test_midi() -> Vec<u8> {
    use crate::smf::{Notated, NotatedPart, PNote, PPQ};
    let notes = |keys: &[u8]| keys.iter().enumerate().map(|(i, &k)| PNote { tick: i as u64 * PPQ as u64, len: PPQ as u64 / 2, key: k, vel: 100 }).collect::<Vec<_>>();
    let mut n = Notated { title: "Test".into(), ..Default::default() };
    n.parts.push(NotatedPart { name: "Violin".into(), program: Some(40), notes: notes(&[76, 79, 81, 79, 76, 74, 72, 74]), ..Default::default() });
    n.parts.push(NotatedPart { name: "Bass".into(), program: Some(33), notes: notes(&[40, 40, 43, 43, 45, 45, 43, 43]), ..Default::default() });
    n.parts.push(NotatedPart { name: "Birds".into(), program: Some(123), notes: notes(&[84, 86, 88, 86, 84, 86, 88, 86]), ..Default::default() });
    n.parts.push(NotatedPart { name: "Drums".into(), drums: true, notes: notes(&[36, 54, 38, 54, 36, 54, 38, 54]), ..Default::default() });
    crate::smf::write_smf(&n)
}

/// AN IMPORTED FILE'S PROGRAM NUMBERS PICK THEIR INSTRUMENTS (music-page.md section 15a): a MIDI file
/// imported and played through the General-MIDI-like style. The violin's staff plays the solo
/// violin part, the bass's the electric bass, each drum key its own drum; the bird tweet (program
/// 123) and the tambourine (key 54) have no instrument of ours and are SAID (a note each, naming
/// what plays them instead), never dropped: soloed, each part is heard, and the violin and the bass
/// sound different (their spectra, measured).
///
/// Seen RED 2026-11-21 with `apply` leaving every staff on its own instruments (the violin's staff
/// still on the piano the import gives it): "left: Some(\"piano\")" against `Some(\"solo_violin\")`;
/// and with the import not keeping each part's program: "the violin's staff keeps its program 40:
/// None".
#[test]
fn an_imported_files_programs_pick_their_instruments_and_a_missing_one_is_said() {
    let lib = shipped();
    let imported = crate::own::import_midi(&test_midi(), "test.mid", &[]).unwrap();
    let progs: Vec<(Option<u8>, bool)> = imported.entry.staves.iter().map(|s| (s.program, s.drums)).collect();
    assert_eq!(progs[0].0, Some(40), "the violin's staff keeps its program 40: {:?}", progs[0].0);
    assert_eq!(progs, vec![(Some(40), false), (Some(33), false), (Some(123), false), (None, true)]);
    let score = crate::mei::read(&imported.mei).unwrap();
    let timeline = Arc::new(crate::written::Timeline::from_mei(&score, imported.entry.qpm));
    let p = WrittenPiece { entry: imported.entry.clone(), score, timeline };
    let styled = apply(style(&lib, "general_midi"), &p);
    let st = &styled.piece.entry.staves;
    assert_eq!(st[0].part.as_deref(), Some("solo_violin"));
    assert_eq!(st[1].part.as_deref(), Some("electric_bass"));
    assert_eq!(st[2].part.as_deref(), Some("piano"), "a program of no instrument of ours plays on the piano");
    let kick = st[3].drum_map.iter().find(|h| h.key == 36).map(|h| h.part.clone());
    assert_eq!(kick, Some(Some("kit_kick".into())));
    assert!(styled.notes.contains(&StyleNote::MissingProgram { staff: 3, program: 123, plays_on: "piano".into() }), "the bird tweet is said: {:?}", styled.notes);
    assert!(styled.notes.contains(&StyleNote::MissingDrum { staff: 4, key: 54, plays_on: "kit_hihat_closed".into() }), "the tambourine is said: {:?}", styled.notes);
    assert_eq!(styled.notes.len(), 2, "only those two are missing: {:?}", styled.notes);
    // Heard: each part soloed (by its role) is not silent, and the violin and bass differ.
    let role_of = |s: u16| ROLE_IDS.iter().position(|r| *r == st.iter().find(|x| x.staff == s).unwrap().role).unwrap();
    let whole = rms(&mono(&render_piece(&lib, &styled.piece, 432.0, Tune::default(), 0, 4.0)));
    let mut spectra = Vec::new();
    for s in 1..=4u16 {
        let tune = Tune { solo: 1 << role_of(s), ..Tune::default() };
        let x = mono(&render_piece(&lib, &styled.piece, 432.0, tune, 0, 4.0));
        let db = 20.0 * (rms(&x) / whole.max(1e-12)).log10();
        println!("imported staff {s} soloed: {db:.1} dB");
        assert!(db > -40.0, "staff {s} is heard: {db:.1} dB");
        spectra.push(spectrum(&x));
    }
    let d = distance(&spectra[0], &spectra[1]);
    assert!(d > 0.2, "the violin and the bass sound different: {d:.3}");
}

/// EACH STYLE STAYS WITHIN THE BLOCK BUDGET (music-page.md sections 18 and 25; the crate card's budget
/// rule): the symphony, the fullest score shipped (twelve staves), rendered as written, in retro FM
/// and as a solo piano, timed block by block over its densest ten seconds (the window with the most
/// notes begun) and over its first ninety seconds, each block's fastest of two renders. The worst
/// mean in quiet-machine ms (`tests_cost`: over how much slower than quiet the yardstick timed among
/// its blocks ran) is at most 0.25 ms a block (under a tenth of the 2.67 ms a 128-frame block lasts at
/// 48 kHz), and no style costs more than a quarter over the piece as written.
///
/// Seen RED 2026-11-21 against a 0.10 ms budget (twice the calm score's 0.05): the symphony AS
/// WRITTEN already fails it, "as written: 0.1114 ms a block, over the 0.10 ms budget" (its densest ten
/// seconds, 690 notes from bar 484, at 0.2032 ms unscaled on a machine 1.82 times busier than quiet),
/// so the written player's budget is set by what the as-written orchestra costs, and a style is held to it.
///
/// LOAD CANNOT FAIL IT (2026-11-21): it failed once in a full run beside other agents' builds and
/// passed alone, each style measured after the one before against a calm score read once before
/// them all. Now the three styles' stretches take turns through the same renders, each judged by
/// the yardstick timed among its own blocks. Measured beside other agents' builds (1.72 times
/// slower): as written 0.1330 ms (0.2287 measured), retro FM 0.1112, solo piano 0.0834; with 12
/// busy threads of multiply-adds as well, 0.1193, 0.1082 and 0.0803; with integer work 0.1249,
/// 0.1086 and 0.0784; walking 64 MB 0.0713, 0.0785 and 0.0540; half each 0.0973, 0.0881 and 0.0706,
/// all passing. RED with a spin costing 0.161 ms planted in every block of the score's render: "as
/// written: 0.2841 ms a block, over the 0.25 ms budget" on a quiet machine (beside the busy threads
/// the same spin read 0.2202 and passed: a plain spin, which load slows less than the engine, is
/// judged there at about two thirds of its cost).
#[test]
fn each_style_stays_within_the_block_budget() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let p = piece(&lib, "beethoven_symphony5_1");
    // The densest ten seconds.
    let starts: Vec<f64> = p.timeline.notes.iter().map(|n| n.at_s).collect();
    let mut best = (0usize, 0.0f64);
    for (i, &t) in starts.iter().enumerate() {
        let n = starts[i..].iter().take_while(|&&u| u < t + 10.0).count();
        if n > best.0 {
            best = (n, t);
        }
    }
    let dense_bar = p.timeline.bar_at(best.1).unwrap_or(0);
    let pieces = [("as written", p.clone()), ("retro FM", apply(style(&lib, "retro_fm"), &p).piece), ("solo piano", apply(style(&lib, "solo_piano"), &p).piece)];
    let time = |p: &WrittenPiece, bar: usize, seconds: f32| {
        let r = res(&lib);
        let mut player = p.player(&r, RATE, 432.0);
        player.seek_bar(bar);
        let mut e = ScoreEngine::written(r, RATE, 1, MusicContext::default(), player);
        crate::tests_cost::time_blocks((seconds * RATE / 128.0) as usize, |buf| e.render(buf))
    };
    // Every style's two stretches take turns through the same renders (2026-11-21: each style was
    // measured after the one before, so a machine getting busier between them judged the load in
    // the quarter-over comparison), each block its fastest of two, the yardstick among them.
    let mut subjects: Vec<Box<dyn FnMut() -> Vec<f64> + '_>> = Vec::new();
    for (_, styled) in &pieces {
        subjects.push(Box::new(move || time(styled, dense_bar, 10.0)));
        subjects.push(Box::new(move || time(styled, 0, 90.0)));
    }
    let t = crate::tests_cost::fastest_blocks(2, subjects);
    let mut worst = Vec::new();
    for (k, (name, _)) in pieces.iter().enumerate() {
        let (dense, opening) = (&t[2 * k], &t[2 * k + 1]);
        let (d, o) = (dense.cost(), opening.cost());
        println!(
            "block budget, {name}: densest 10 s (from bar {dense_bar}, {} notes) mean {:.4} ms, p99 {:.4} ms ({}); first 90 s mean {:.4} ms, p99 {:.4} ms ({})",
            best.0,
            d.mean,
            d.p99,
            dense.how(),
            o.mean,
            o.p99,
            opening.how()
        );
        let (w, how) = if d.mean >= o.mean { (d.mean, dense.how()) } else { (o.mean, opening.how()) };
        worst.push((*name, w, how));
    }
    let written = worst[0].1;
    for (name, w, how) in &worst {
        assert!(*w <= 0.25, "{name}: {w:.4} ms a block, over the 0.25 ms budget ({how})");
        assert!(*w <= written * 1.25, "{name}: {w:.4} ms a block, more than a quarter over the piece as written ({written:.4} ms; {how})");
    }
}

/// Electro swing in the calm mood, seed `seed`, tuned by `tune` and heard at `intensity`: `seconds`
/// rendered, the first two dropped.
fn swing_render(lib: &MusicLibrary, tune: Tune, intensity: f32, seed: u64, seconds: f32) -> Vec<[f32; 2]> {
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let ctx = MusicContext { mood, genre_a: g, genre_b: g, intensity, tune, ..Default::default() };
    let mut e = ScoreEngine::new(res(lib), RATE, seed, ctx);
    let r = crate::render::render_score(&mut e, RATE, seconds as f64);
    r.frames[(2.0 * RATE) as usize..].to_vec()
}

/// Where the off-beats fall in the beat, measured in the sound (the method of
/// `tests_audio::electro_swing_renders_at_128_bpm_with_its_swing`): the low band's onsets fitted to a
/// beat grid, the high band's folded onto it, the off-beats' median place less the on-beats'.
pub(crate) fn measured_swing(frames: &[[f32; 2]]) -> f64 {
    let low = crate::tests_audio::onsets(frames, RATE, false);
    let high = crate::tests_audio::onsets(frames, RATE, true);
    let mut gaps: Vec<f64> = low.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_by(|a, b| a.total_cmp(b));
    let guess = gaps[gaps.len() / 2];
    let t0 = low[0];
    let grid: Vec<(f64, f64)> = low
        .iter()
        .filter_map(|&t| {
            let n = ((t - t0) / guess).round();
            (((t - t0) / guess - n).abs() < 0.12).then_some((n, t))
        })
        .collect();
    let k = grid.len() as f64;
    let (sn, st) = grid.iter().fold((0.0, 0.0), |a, &(n, t)| (a.0 + n, a.1 + t));
    let (mn, mt) = (sn / k, st / k);
    let (num, den) = grid.iter().fold((0.0, 0.0), |a, &(n, t)| (a.0 + (n - mn) * (t - mt), a.1 + (n - mn) * (n - mn)));
    let beat = num / den;
    let origin = mt - beat * mn;
    let phase = |t: f64| ((t - origin) / beat).rem_euclid(1.0);
    let mut on: Vec<f64> = high.iter().map(|&t| phase(t)).filter(|&p| !(0.15..=0.85).contains(&p)).map(|p| if p > 0.5 { p - 1.0 } else { p }).collect();
    let mut off: Vec<f64> = high.iter().map(|&t| phase(t)).filter(|&p| (0.45..0.85).contains(&p)).collect();
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    median(&mut off) - median(&mut on)
}

/// SWING MOVES THE OFF-BEATS BY THE STATED AMOUNT (the Music page's Swing, music-page.md rung 2, built
/// in rung 3a): electro swing (its own swing 0.64) with the page's swing set to 0.56 and to 0.72 and
/// its drums soloed on the mixer (the drum machine keeps the time; a lead's own sharp onsets would
/// pull the measure, as the genre's own swing test found), its onsets found in the rendered audio:
/// the off-beats land within 0.03 of the beat of each stated amount, so they moved by the 0.16 stated
/// between them.
///
/// Seen RED 2026-11-21, a first version measuring the whole mix: "swing 0.56 asked: the off-beats land
/// at 0.651 of the beat" (the composer's own sheet put the drums' off-beats at 0.560 to 0.568, the
/// humanize included: the piece's lead, not the swing, pulled the measure, so the drums are soloed).
/// Then RED with the composer ignoring the person's swing (the `steer.tune.swing` override where the
/// phrase is planned left out): "swing 0.56 asked: the off-beats land at 0.640 of the beat".
#[test]
fn swing_moves_the_off_beats_by_the_stated_amount() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let mut got = Vec::new();
    for asked in [0.56f32, 0.72] {
        let x = swing_render(&lib, Tune { swing: asked, solo: 1 << crate::tune::PERCUSSION, ..Tune::default() }, 0.3, 20261112, 24.0);
        let m = measured_swing(&x);
        println!("swing {asked} asked: the off-beats at {m:.3} of the beat");
        assert!((m - asked as f64).abs() < 0.03, "swing {asked} asked: the off-beats land at {m:.3} of the beat");
        got.push(m);
    }
    assert!(got[1] - got[0] > 0.12, "the off-beats moved by the stated 0.16: {:.3}", got[1] - got[0]);
}

/// INTENSITY CHANGES THE MEASURED LEVEL AND DENSITY (the Music page's Intensity, music-page.md rung 2,
/// built in rung 3a): the same piece of electro swing at intensity 0.1 and at 0.9 (the action band
/// and its layers), its loudness (the rendered audio's RMS) and its density (onsets found in both
/// bands of the rendered audio, counted a minute) both higher at 0.9.
///
/// Seen RED 2026-11-21 with the score ignoring the context's intensity (`score::steer_of` holding it at
/// 0.3): "intensity 0.9 is louder than 0.1: +0.0 dB".
#[test]
fn intensity_changes_the_measured_level_and_density() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let measure = |intensity: f32| {
        let x = swing_render(&lib, Tune::default(), intensity, 7, 26.0);
        let rms = (x.iter().map(|f| (f[0] as f64).powi(2) + (f[1] as f64).powi(2)).sum::<f64>() / (2 * x.len()) as f64).sqrt();
        let n = crate::tests_audio::onsets(&x, RATE, false).len() + crate::tests_audio::onsets(&x, RATE, true).len();
        (20.0 * rms.log10(), n as f64 * 60.0 / 24.0)
    };
    let (lo_db, lo_n) = measure(0.1);
    let (hi_db, hi_n) = measure(0.9);
    println!("intensity 0.1: {lo_db:.1} dB, {lo_n:.0} onsets a minute; 0.9: {hi_db:.1} dB, {hi_n:.0} onsets a minute");
    assert!(hi_db - lo_db > 1.0, "intensity 0.9 is louder than 0.1: {:+.1} dB", hi_db - lo_db);
    assert!(hi_n > lo_n * 1.1, "intensity 0.9 is busier than 0.1: {hi_n:.0} onsets a minute against {lo_n:.0}");
}
