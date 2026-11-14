//! Pieces: variety by seed (docs/design/music.md section 7c; the operator, 2026-11-13: "we don't
//! want the same electro swing song for hours on end, we definitely want variety to the music. Even
//! different chords and what not."). The composer runs dry (no sound), so hours pass in seconds.
//! Each test was seen failing first; the red is in its comment.

use crate::composer::{Composer, Event, Steer};
use crate::mood::RoleKind;
use crate::piece::GenMotif;
use crate::resolved::Resolved;
use crate::tests::{repeated_phrase, repeated_stretch, run, shipped, steer, Recorder};
use crate::tuning::Mode;
use crate::SampleLibrary;

/// What a piece played, summed up: its first key, its chord walk (chord indices, in order), its
/// riffs, its form, its tempo and the instrument its melody played.
#[derive(Debug, PartialEq)]
struct Heard {
    key: i32,
    chords: Vec<u16>,
    motifs: Vec<GenMotif>,
    form: Vec<u8>,
    tempo: f32,
    melody: Option<u16>,
}

fn play(res: &Resolved, st: Steer, seed: u64, seconds: f64) -> (Heard, Recorder) {
    let mut c = Composer::new(48000.0, seed, st);
    let mut rec = Recorder::default();
    run(&mut c, res, seconds, &mut rec);
    let key = rec.events.iter().find_map(|e| if let Event::Phrase { key_pc, .. } = e { Some(*key_pc) } else { None }).unwrap_or(-1);
    let chords = rec.events.iter().filter_map(|e| if let Event::Chord { chord, .. } = e { Some(*chord) } else { None }).collect();
    let heard = Heard { key, chords, motifs: c.piece_motifs().to_vec(), form: c.piece_form().to_vec(), tempo: c.tempo(), melody: c.role_instruments()[RoleKind::Melody.index()] };
    (heard, rec)
}

/// TWO SEEDS, TWO PIECES; ONE SEED, ONE PIECE: eight electro swing seeds, three minutes each, give
/// at least four different keys, eight different chord walks and eight different sets of riffs, and
/// more than one form and lead voice; seed 7 played twice is the same piece twice, note for note.
/// RED first (2026-11-13) with the piece's key draw taken out (every piece in the genre's first
/// key): "two seeds give different keys: [2, 2, 2, 2, 2, 2, 2, 2]".
#[test]
fn two_seeds_make_two_pieces_and_one_seed_the_same_piece_twice() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let st = steer(&lib, "mothership_calm", "electro_swing");
    let pieces: Vec<Heard> = (1..=8u64).map(|s| play(&res, st, s, 180.0).0).collect();
    let mut keys: Vec<i32> = pieces.iter().map(|p| p.key).collect();
    println!("keys {keys:?}; forms {:?}; melody {:?}", pieces.iter().map(|p| p.form.clone()).collect::<Vec<_>>(), pieces.iter().map(|p| p.melody).collect::<Vec<_>>());
    let shown = keys.clone();
    keys.sort();
    keys.dedup();
    assert!(keys.len() >= 4, "two seeds give different keys: {shown:?}");
    for i in 0..pieces.len() {
        for j in 0..i {
            assert_ne!(pieces[i].chords, pieces[j].chords, "seeds {} and {} walked the same chords", i + 1, j + 1);
            assert_ne!(pieces[i].motifs, pieces[j].motifs, "seeds {} and {} made the same riffs", i + 1, j + 1);
        }
    }
    assert!(pieces.iter().all(|p| p.motifs.len() == 5), "each piece makes the genre's five riffs");
    let mut forms: Vec<Vec<u8>> = pieces.iter().map(|p| p.form.clone()).collect();
    forms.sort();
    forms.dedup();
    let mut leads: Vec<Option<u16>> = pieces.iter().map(|p| p.melody).collect();
    leads.sort();
    leads.dedup();
    assert!(forms.len() > 1 && leads.len() > 1, "pieces vary their form ({forms:?}) and their lead voice ({leads:?})");
    // One seed, the same piece twice: every note and every event.
    let (a, ra) = play(&res, st, 7, 300.0);
    let (b, rb) = play(&res, st, 7, 300.0);
    assert_eq!(a, b, "one seed, one piece");
    let notes = |r: &Recorder| r.notes.iter().map(|(t, n)| (((t * 1000.0).round() as i64), n.midi as i32, n.inst)).collect::<Vec<_>>();
    assert!(!ra.notes.is_empty() && notes(&ra) == notes(&rb), "one seed plays the same notes twice");
}

/// A run of pieces as the runtime plays them: a new piece (the next seed, as engine/music.rs makes
/// it) every `piece_s` seconds, the events laid end to end.
fn pieces_for(res: &Resolved, st: Steer, first_seed: u64, total_s: f64, piece_s: f64) -> Recorder {
    let mut all = Recorder::default();
    let mut t0 = 0.0;
    let mut n = 0u64;
    while t0 < total_s {
        let len = piece_s.min(total_s - t0);
        let (_, rec) = play(res, st, first_seed.wrapping_add(n.wrapping_mul(0x9e37_79b9)), len);
        let shift = |t: f64| t + t0;
        for e in rec.events {
            all.events.push(match e {
                Event::Phrase { hash, at_s, mood, key_pc, section, chorus, returning } => Event::Phrase { hash, at_s: shift(at_s), mood, key_pc, section, chorus, returning },
                Event::Chord { chord, pcs, at_s, beats } => Event::Chord { chord, pcs, at_s: shift(at_s), beats },
                Event::Voicing { role, notes, n, at_s } => Event::Voicing { role, notes, n, at_s: shift(at_s) },
                Event::Motif { role, first_note, at_s } => Event::Motif { role, first_note, at_s: shift(at_s) },
                Event::Band { band, at_s } => Event::Band { band, at_s: shift(at_s) },
            });
        }
        t0 += len;
        n += 1;
    }
    all
}

/// AN HOUR REPEATS NO TWO-MINUTE STRETCH, with a new piece every four minutes (the default) and with
/// one piece the whole hour ("Never"), in every genre: the chord and motif events of every
/// two-minute window starting at a chord change hash differently from every other window's, and no
/// phrase repeats one in the ten minutes before it except a section returning on purpose. The
/// detector is shown able to fail on every run (a positive control): a piece that loops its form
/// verbatim, its tune and chords the same every time through, must be caught repeating. On
/// 2026-11-13 the test still passed with the reharmonised second halves taken out (the improvised
/// riffs alone vary an hour enough), so its red is that control: the detector fails on the looping
/// piece, every run.
#[test]
fn an_hour_of_pieces_repeats_no_two_minute_stretch() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    {
        let mut c = Composer::new(48000.0, 11, steer(&lib, "mothership_calm", "electro_swing"));
        c.play_verbatim();
        let mut rec = Recorder::default();
        run(&mut c, &res, 1200.0, &mut rec);
        assert!(repeated_stretch(&rec, 1200.0).0.is_some(), "a piece looping its form verbatim is caught repeating");
    }
    for genre in ["electro_swing", "lofi", "mothership_calm"] {
        let st = steer(&lib, "mothership_calm", genre);
        for (label, piece_s) in [("a piece every 4 minutes", 240.0), ("one piece", 3600.0)] {
            let rec = pieces_for(&res, st, 11, 3600.0, piece_s);
            let (stretch, checked) = repeated_stretch(&rec, 3600.0);
            if let Some((t0, prev)) = stretch {
                panic!("{genre}, {label}: the two minutes from {t0:.1} s repeat the two minutes from {prev:.1} s");
            }
            assert!(checked > 100, "{genre}, {label}: {checked} windows checked");
            if let Some((i, t, t2)) = repeated_phrase(&rec, 600.0) {
                panic!("{genre}, {label}: phrase {i} at {t:.1} s repeats the one from {t2:.1} s");
            }
        }
    }
}

/// EVERY PIECE STAYS INSIDE ITS GENRE'S RULES: twenty electro swing pieces of four minutes each play
/// at the genre's 128 bpm, in its keys only, in one of its forms; every chord is one of its chords
/// or a chord its grammar derives, sounding exactly that chord's notes over the key; every riff note
/// of the melody and its answer is in the minor blues scale of the key; the melody and the answer
/// are voices from its leads, never the same one. RED first (2026-11-13) against the riffs' plain
/// clamp to the register (a riff note past the answer's register landed between scale notes):
/// "seed 1: a riff note 65 (F) is not in the A blues scale".
#[test]
fn every_piece_stays_inside_its_genres_rules() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let gi = lib.genre_index("electro_swing").unwrap();
    let st = steer(&lib, "mothership_calm", "electro_swing");
    let g = &res.genres[gi];
    let h = g.progressions.as_ref().expect("electro swing has its own progressions");
    let key_pcs: Vec<i32> = h.keys.iter().map(|k| k.0).collect();
    let forms: Vec<Vec<u8>> = g.forms.iter().map(|f| f.0.clone()).collect();
    let lead_insts = |role: RoleKind| -> Vec<u16> { g.leads.iter().filter(|l| l.0 == role.index()).map(|l| l.1.inst).collect() };
    let (melodies, answers) = (lead_insts(RoleKind::Melody), lead_insts(RoleKind::Bells));
    let blues = |key: i32, midi: i32| Mode::Blues.contains(midi - key);
    for seed in 1..=20u64 {
        let mut c = Composer::new(48000.0, seed, st);
        let mut rec = Recorder::default();
        let mut key = -1;
        let mut t = 0.0;
        while t < 240.0 {
            run(&mut c, &res, 2.0, &mut rec);
            t += 2.0;
            let r = c.role_instruments();
            if let (Some(m), Some(a)) = (r[RoleKind::Melody.index()], r[RoleKind::Bells.index()]) {
                assert!(melodies.contains(&m) && answers.contains(&a) && m != a, "seed {seed}: the melody {m} and the answer {a} are two of the genre's lead voices");
            }
        }
        assert!((c.tempo() - 128.0).abs() < 1e-3, "seed {seed}: {} bpm", c.tempo());
        assert!(forms.iter().any(|f| f.as_slice() == c.piece_form()), "seed {seed}: form {:?}", c.piece_form());
        let mut ni = 0;
        for e in &rec.events {
            match *e {
                Event::Phrase { key_pc, at_s, .. } => {
                    assert!(key_pcs.contains(&key_pc), "seed {seed}: key {key_pc} is not one of the genre's");
                    key = key_pc;
                    // The riff notes of this phrase (from here to the next phrase) are in its scale.
                    let next = rec.events.iter().find_map(|x| if let Event::Phrase { at_s: a, .. } = x { (*a > at_s).then_some(*a) } else { None }).unwrap_or(f64::INFINITY);
                    while ni < rec.notes.len() && rec.notes[ni].0 < next {
                        let (_, n) = &rec.notes[ni];
                        if (n.role as usize == RoleKind::Melody.index() || n.role as usize == RoleKind::Bells.index()) && rec.notes[ni].0 >= at_s {
                            let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                            assert!(blues(key, n.midi as i32), "seed {seed}: a riff note {} ({}) is not in the {} blues scale", n.midi, names[(n.midi as i32).rem_euclid(12) as usize], names[key.rem_euclid(12) as usize]);
                        }
                        ni += 1;
                    }
                }
                Event::Chord { chord, pcs, .. } => {
                    let ch = h.chords.get(chord as usize).unwrap_or_else(|| panic!("seed {seed}: chord {chord} is not one of the genre's or its grammar's"));
                    let want = ch.notes.iter().fold(0u16, |acc, &iv| acc | 1 << (key + ch.root + iv).rem_euclid(12));
                    assert_eq!(pcs, want, "seed {seed}: chord {chord} sounds its own notes over the key");
                }
                _ => {}
            }
        }
    }
}
