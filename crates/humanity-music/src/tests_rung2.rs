//! The Music page's rung 2, the crate's half (docs/design/music-page.md sections 7, 8, 12 and 15a):
//! the part meters read from the sound each part makes, Solo, a seek in a written piece, and written
//! music out and in as MEI and Standard MIDI Files (crate::smf, crate::mei_write, crate::own). Each
//! test measures what the engine or the files PRODUCE (rendered samples, the notes read back by the
//! reader), never the value a control was set to.

use crate::mei::{read, Frac, MeiNote};
use crate::scores::load_piece;
use crate::sheet::transcribe;
use crate::smf::{read_smf, write_smf, Notated, PPQ};
use crate::tests::{data_dir, shipped};
use crate::tune::Tune;
use crate::{MusicContext, MusicLibrary, Resolved, SampleLibrary, ScoreEngine};

fn res(lib: &MusicLibrary) -> Resolved {
    Resolved::new(lib, &SampleLibrary::new()).expect("resolve")
}

/// Electro swing in the calm mood with the page open (the meters on), tuned by `tune`.
fn swing_ctx(lib: &MusicLibrary, tune: Tune) -> MusicContext {
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    MusicContext { mood, genre_a: g, genre_b: g, analyse: true, tune, ..Default::default() }
}

/// Render `seconds` of seed 5: every sample, and each role's meter averaged over the blocks after the
/// first two seconds.
fn render(lib: &MusicLibrary, ctx: MusicContext, seconds: f32) -> (Vec<[f32; 2]>, [f64; 8]) {
    let mut e = ScoreEngine::new(res(lib), 48000.0, 5, ctx);
    let mut buf = vec![[0.0f32; 2]; 128];
    let mut all = Vec::new();
    let mut sum = [0.0f64; 8];
    let mut count = 0usize;
    let blocks = (seconds * 48000.0 / 128.0) as usize;
    for b in 0..blocks {
        e.render(&mut buf);
        all.extend_from_slice(&buf);
        if b as f32 * 128.0 / 48000.0 >= 2.0 {
            let r = e.status().roles;
            for k in 0..8 {
                sum[k] += r[k] as f64;
            }
            count += 1;
        }
    }
    (all, sum.map(|s| s / count.max(1) as f64))
}

fn rms(x: &[[f32; 2]]) -> f64 {
    (x.iter().map(|f| (f[0] as f64).powi(2) + (f[1] as f64).powi(2)).sum::<f64>() / (2 * x.len().max(1)) as f64).sqrt()
}

const BASS: usize = 1;
const MELODY: usize = 5;

/// THE METERS FOLLOW EACH PART'S REAL LEVEL (music-page.md section 8): the bass's fader 12 dB down
/// moves the bass meter down by about 12 dB (a quarter of the amplitude) while the melody's stays
/// where it was, a muted bass reads nothing, and the meter's ratio is the ratio of the bass's own
/// sound, measured from the rendered audio with the bass soloed (the music's whole output, its
/// reverb included).
///
/// Seen RED 2026-11-20 twice. Its first run found a real defect: "the meter moves as the bass's own
/// sound does: meter 0.251, sound 0.985" (soloing the bass left the genre's drum machine, role 255,
/// playing: tune::part_of now puts it in the drums). Then with the meters made blind to the fader
/// (each role's energy divided by its gain, as a meter reading the notes sent rather than the sound
/// made would be): "the bass fader 12 dB down moves its meter by a quarter: 1.000".
#[test]
fn the_meters_follow_each_parts_real_level() {
    let lib = shipped();
    let open = swing_ctx(&lib, Tune::default());
    let mut quiet_t = Tune::default();
    quiet_t.level_db[BASS] = -12.0;
    let (_, m_open) = render(&lib, open, 16.0);
    let (_, m_quiet) = render(&lib, swing_ctx(&lib, quiet_t), 16.0);
    let (_, m_muted) = render(&lib, swing_ctx(&lib, Tune { mute: 1 << BASS, ..Tune::default() }), 16.0);
    assert!(m_open[BASS] > 1e-3 && m_open[MELODY] > 1e-3, "both parts play: bass {} melody {}", m_open[BASS], m_open[MELODY]);
    let ratio = m_quiet[BASS] / m_open[BASS];
    assert!((0.17..0.36).contains(&ratio), "the bass fader 12 dB down moves its meter by a quarter: {ratio:.3}");
    let melody = m_quiet[MELODY] / m_open[MELODY];
    assert!((0.9..1.1).contains(&melody), "the melody's meter stays where it was: {melody:.3}");
    assert_eq!(m_muted[BASS], 0.0, "a muted bass reads nothing");
    // The independent reading: the bass alone, at 0 and at -12 dB, as rendered.
    let (solo_open, _) = render(&lib, swing_ctx(&lib, Tune { solo: 1 << BASS, ..Tune::default() }), 16.0);
    let mut solo_quiet = Tune { solo: 1 << BASS, ..Tune::default() };
    solo_quiet.level_db[BASS] = -12.0;
    let (solo_quiet_out, _) = render(&lib, swing_ctx(&lib, solo_quiet), 16.0);
    let heard = rms(&solo_quiet_out) / rms(&solo_open);
    assert!((ratio / heard - 1.0).abs() < 0.3, "the meter moves as the bass's own sound does: meter {ratio:.3}, sound {heard:.3}");
}

/// SOLO SILENCES THE OTHER PARTS' OUTPUT (music-page.md section 8): with the bass soloed the music
/// renders sample for sample as it does with every other part muted, unlike the music unsoloed, and
/// every other part's meter (read from its sound) is still.
///
/// Seen RED 2026-11-20 with the drum machine and the risers (roles 254 and 255) in no part, as before
/// rung 2: "with the bass soloed, its fader 12 dB down takes the whole sound down by a quarter: 0.984"
/// (the sample-for-sample comparison alone stayed green: muting every other part leaked the same).
#[test]
fn solo_silences_the_other_parts_in_the_rendered_audio() {
    let lib = shipped();
    let (full, _) = render(&lib, swing_ctx(&lib, Tune::default()), 8.0);
    let (solo, meters) = render(&lib, swing_ctx(&lib, Tune { solo: 1 << BASS, ..Tune::default() }), 8.0);
    let (alone, _) = render(&lib, swing_ctx(&lib, Tune { mute: !(1u8 << BASS), ..Tune::default() }), 8.0);
    let first_diff = solo.iter().zip(&alone).position(|(a, b)| a != b);
    assert_eq!(first_diff, None, "the bass soloed renders as every other part muted");
    let others: Vec<[f32; 2]> = full.iter().zip(&solo).map(|(a, b)| [a[0] - b[0], a[1] - b[1]]).collect();
    assert!(rms(&others) > 0.2 * rms(&full), "the rest of the band is a good part of the sound unsoloed: {} of {}", rms(&others), rms(&full));
    for (k, m) in meters.iter().enumerate() {
        if k != BASS {
            assert_eq!(*m, 0.0, "part {k}'s meter is still while the bass is soloed");
        }
    }
    assert!(meters[BASS] > 1e-3, "the bass's meter moves: {}", meters[BASS]);
    // Nothing else sounds: the bass's fader 12 dB down takes the whole output down by 12 dB (a
    // quarter of the amplitude), as it could not if any other part still played.
    let mut quiet = Tune { solo: 1 << BASS, ..Tune::default() };
    quiet.level_db[BASS] = -12.0;
    let (quiet_out, _) = render(&lib, swing_ctx(&lib, quiet), 8.0);
    let ratio = rms(&quiet_out) / rms(&solo);
    assert!((0.17..0.36).contains(&ratio), "with the bass soloed, its fader 12 dB down takes the whole sound down by a quarter: {ratio:.3}");
}

/// A SEEK MOVES THE WRITTEN PIECE TO THE BAR (music-page.md section 7): Für Elise two seconds in, a
/// seek to bar 21 (index 20) moves the player's clock to that bar's start as the timeline times it,
/// and the same seek number sent again (the context goes every frame) does not move it back.
///
/// Seen RED 2026-11-20 with `ScoreEngine::set_context`'s seek left out: "the clock stands at bar
/// 20's start (22.92 s): 2.00".
#[test]
fn a_seek_moves_the_written_piece_to_the_bar() {
    let lib = shipped();
    let piece = load_piece(&data_dir(), lib.score("fur_elise").unwrap()).unwrap();
    let r = res(&lib);
    let player = piece.player(&r, 48000.0, 432.0);
    let ctx = MusicContext::default();
    let mut e = ScoreEngine::written(r, 48000.0, 1, ctx, player);
    let mut buf = vec![[0.0f32; 2]; 128];
    for _ in 0..(2.0 * 48000.0 / 128.0) as usize {
        e.render(&mut buf);
    }
    let want = piece.timeline.bars[20].at_s;
    e.set_context(MusicContext { seek: 1, seek_bar: 20, ..ctx });
    e.render(&mut buf);
    let s = e.status();
    assert!((s.seconds - want).abs() < 0.01, "the clock stands at bar 20's start ({want:.2} s): {:.2}", s.seconds);
    assert_eq!(s.bar, 20);
    for _ in 0..100 {
        e.set_context(MusicContext { seek: 1, seek_bar: 20, ..ctx });
        e.render(&mut buf);
    }
    assert!(e.status().seconds > want + 0.2, "the same seek again does not hold it there: {:.2}", e.status().seconds);
}

/// The notes a reader found, by staff, sounding (grace notes aside: a MIDI file holds them as short
/// notes, and the round trip writes them so), as (start, pitch, length) in whole notes.
fn sounding(notes: &[MeiNote]) -> Vec<(f64, i32, f64)> {
    let f = |x: Frac| x.n as f64 / x.d as f64;
    let mut v: Vec<(f64, i32, f64)> = notes.iter().filter(|n| !n.grace).map(|n| (f(n.at), n.midi.round() as i32, f(n.dur))).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    v
}

/// EXPORT THEN IMPORT ROUND-TRIPS A WRITTEN PIECE'S NOTES (music-page.md sections 12 and 15a): Ode
/// to Joy and Für Elise written out as a Standard MIDI File, read back by the importer (midly, then
/// notated as MEI by our writer, then read by our reader as the player reads it), give the same notes:
/// every one at exactly its place, pitch and length (a triplet written as a triplet, not moved to the
/// nearest 64th), and no note more.
///
/// Seen RED 2026-11-20: first with a tolerance of a 128th note, which stayed green with the triplets
/// turned off in the writer (each moved to the nearest 64th), so it is exact now; exact, it failed on
/// a real defect: "fur_elise: 1023 notes came back for 1038 (first missing: Some((37.125, 74,
/// 0.041666666666666664)))", a triplet in the first eighth of a 3/8 bar, while the writer only wrote
/// whole quarters as triplets (now the meter's beat: an eighth in 3/8).
#[test]
fn export_then_import_round_trips_a_written_pieces_notes() {
    let lib = shipped();
    for id in ["ode_to_joy", "fur_elise"] {
        let entry = lib.score(id).unwrap();
        let piece = load_piece(&data_dir(), entry).unwrap();
        let bytes = write_smf(&Notated::from_mei(&piece.score, entry));
        let imported = crate::own::import_midi(&bytes, &format!("{id}.mid"), &[]).unwrap_or_else(|e| panic!("{id}: {e}"));
        let back = read(&imported.mei).unwrap();
        assert!(back.problems.is_empty(), "{id}: the written MEI reads without a problem: {:?}", back.problems);
        let want = sounding(&piece.score.notes);
        let got = sounding(&back.notes);
        let mut used = vec![false; got.len()];
        let mut missing = Vec::new();
        for w in &want {
            match got.iter().enumerate().position(|(i, g)| !used[i] && g.1 == w.1 && (g.0 - w.0).abs() <= 1e-9 && (g.2 - w.2).abs() <= 1e-9) {
                Some(i) => used[i] = true,
                None => missing.push(*w),
            }
        }
        assert!(missing.is_empty(), "{id}: {} notes came back for {} (first missing: {:?})", want.len() - missing.len(), want.len(), missing.first());
        let grace = piece.score.notes.iter().filter(|n| n.grace).count();
        assert!(got.len() <= want.len() + grace, "{id}: no note more than it had: {} for {} and {grace} grace notes", got.len(), want.len());
        assert_eq!(imported.entry.staves.len(), piece.score.staves.len(), "{id}: a staff for each part");
    }
}

/// A COMPOSED PIECE EXPORTS AS MIDI AND AS MEI WITH THE NOTES THE ROLL DREW (music-page.md section
/// 12): thirty seconds of electro swing (swung, so triplet quarters are written) written down by the
/// composer run dry, out as MIDI and read back by midly gives every note in its part; out as MEI and
/// read back by the reader gives every note at its pitch, within a 64th note of where the MIDI file
/// put it.
#[test]
fn a_composed_piece_exports_as_midi_and_mei_with_its_notes() {
    let lib = shipped();
    let ctx = swing_ctx(&lib, Tune::default());
    let sheet = transcribe(&res(&lib), 5, &ctx, 48000.0, 30.0);
    let names: Vec<String> = crate::tune::ROLE_IDS.iter().map(|s| s.to_string()).collect();
    let n = Notated::from_sheet(&sheet, &names, "Electro swing, seed 5");
    let total: usize = n.parts.iter().map(|p| p.notes.len()).sum();
    assert!(total > 50, "the sheet has notes: {total}");
    let back = read_smf(&write_smf(&n)).unwrap();
    assert_eq!(back.parts.len(), n.parts.len(), "a track for each part that plays");
    for (a, b) in n.parts.iter().zip(&back.parts) {
        assert_eq!(a.name, b.name);
        let key = |p: &crate::smf::PNote| (p.tick, p.key);
        let mut x: Vec<_> = a.notes.iter().map(key).collect();
        let mut y: Vec<_> = b.notes.iter().map(key).collect();
        x.sort();
        y.sort();
        x.dedup();
        y.dedup();
        assert_eq!(x, y, "{}: the MIDI file holds the part's notes", a.name);
    }
    assert!(!back.markers.is_empty() && !back.texts.is_empty(), "the section letters and the chord symbols travel");
    let mei = crate::mei_write::write_mei(&n, "test");
    let score = read(&mei).unwrap();
    assert!(score.problems.is_empty(), "{:?}", score.problems.first());
    let whole = 4.0 * PPQ as f64;
    let mut missing = 0;
    for p in &n.parts {
        for note in &p.notes {
            let at = note.tick as f64 / whole;
            if !score.notes.iter().any(|m| m.midi.round() as u8 == note.key && ((m.at.n as f64 / m.at.d as f64) - at).abs() <= 1.0 / 64.0) {
                missing += 1;
            }
        }
    }
    assert!(missing * 100 <= total, "at most one note in a hundred lost to the 64th grid: {missing} of {total}");
}

/// A MALFORMED MIDI FILE IS REFUSED, IN WORDS (music-page.md section 15a): text, a header cut short,
/// a file timed in film frames, a file with no notes and a format 2 file each come back as a sentence
/// a person can read, never a panic or an empty piece.
///
/// Seen RED 2026-11-20 with `read_smf`'s first check removed: "text is refused as not MIDI: it could
/// not be read as a MIDI file: invalid midi: not a midi file" (midly's words, not ours).
#[test]
fn a_malformed_midi_file_is_refused_in_words() {
    let refused = |b: &[u8]| read_smf(b).expect_err("refused");
    assert!(refused(b"This is a shopping list, not music.").contains("not a MIDI file"), "text is refused as not MIDI: {}", refused(b"This is a shopping list, not music."));
    let cut = b"MThd\x00\x00\x00\x06\x00\x01\x00\x02\x01\xe0MTrk\x00\x00\x10\x00\x00\x90";
    assert!(refused(cut).contains("could not be read") || refused(cut).contains("no notes"), "a header cut short: {}", refused(cut));
    let mut empty = Vec::new();
    let smf = midly::Smf { header: midly::Header::new(midly::Format::Parallel, midly::Timing::Metrical(480.into())), tracks: vec![vec![midly::TrackEvent { delta: 0.into(), kind: midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack) }]] };
    smf.write_std(&mut empty).unwrap();
    assert!(refused(&empty).contains("no notes"), "{}", refused(&empty));
    let mut frames = Vec::new();
    let smf = midly::Smf { header: midly::Header::new(midly::Format::SingleTrack, midly::Timing::Timecode(midly::Fps::Fps25, 40)), tracks: vec![vec![midly::TrackEvent { delta: 0.into(), kind: midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack) }]] };
    smf.write_std(&mut frames).unwrap();
    assert!(refused(&frames).contains("film frames"), "{}", refused(&frames));
    let mut songs = Vec::new();
    let smf = midly::Smf { header: midly::Header::new(midly::Format::Sequential, midly::Timing::Metrical(480.into())), tracks: vec![vec![midly::TrackEvent { delta: 0.into(), kind: midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack) }]] };
    smf.write_std(&mut songs).unwrap();
    assert!(refused(&songs).contains("format 2"), "{}", refused(&songs));
    assert!(crate::own::import_midi(b"nonsense", "x.mid", &[]).is_err());
}

/// AN IMPORTED PIECE IS KEPT IN THE PERSON'S FOLDER AND PLAYS FROM THERE: kept, listed again from
/// their own index (marked with its folder), read and timed by the same loader the player uses, given
/// a new id when one is taken, and removed with its file.
#[test]
fn an_imported_piece_is_kept_in_the_persons_folder_and_read_from_there() {
    let dir = std::env::temp_dir().join(format!("hos-music-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib = shipped();
    let entry = lib.score("ode_to_joy").unwrap();
    let piece = load_piece(&data_dir(), entry).unwrap();
    let bytes = write_smf(&Notated::from_mei(&piece.score, entry));
    let mut pieces = Vec::new();
    let first = crate::own::import_midi(&bytes, "Ode.mid", &[]).unwrap();
    let kept = crate::own::keep(&dir, &mut pieces, first).unwrap();
    let again = crate::own::import_midi(&bytes, "Ode.mid", &[kept.id.clone()]).unwrap();
    assert_ne!(again.entry.id, kept.id, "a second import gets an id of its own");
    let listed = crate::own::load(&dir).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].folder.as_deref(), Some(dir.as_path()));
    let played = load_piece(&data_dir(), &listed[0]).unwrap();
    assert_eq!(played.score.notes.len(), piece.score.notes.len(), "the kept piece plays every note");
    assert!((listed[0].length_s - played.timeline.length_s as f32).abs() < 0.5, "its length is the length it plays");
    crate::own::remove(&dir, &mut pieces, &kept.id).unwrap();
    assert!(crate::own::load(&dir).unwrap().is_empty());
    assert!(!dir.join(&kept.file).exists(), "its file goes with it");
    let _ = std::fs::remove_dir_all(&dir);
}
