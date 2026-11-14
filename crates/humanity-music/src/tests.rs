//! The music crate's tests that need no sound rendered: the pitch table, the shipped data, the
//! places, the composer's properties over an hour, the genre layer and the intensity bands. Each
//! was seen failing first; the red is in its comment. The rendering tests are in tests_audio.rs.

use std::path::{Path, PathBuf};

use crate::composer::{Composer, Event, Sink, Steer};
use crate::library::MusicLibrary;
use crate::mood::RoleKind;
use crate::places::Place;
use crate::resolved::Resolved;
use crate::tuning::{note_hz, parse_note, Tuning};
use crate::voice::NoteOn;
use crate::SampleLibrary;

pub(crate) fn data_dir() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

pub(crate) fn shipped() -> MusicLibrary {
    MusicLibrary::load(&data_dir()).unwrap_or_else(|e| panic!("the shipped music data: {e}"))
}

/// A sink that writes everything down.
#[derive(Default)]
pub(crate) struct Recorder {
    pub notes: Vec<(f64, NoteOn)>,
    pub events: Vec<Event>,
    pub now_s: f64,
}

impl Sink for Recorder {
    fn note_on(&mut self, note: NoteOn) {
        self.notes.push((self.now_s, note));
    }
    fn note_off(&mut self, _tag: u32) {}
    fn event(&mut self, e: Event) {
        self.events.push(e);
    }
}

/// Run a composer for `seconds`, keeping the recorder's clock in step (the dry run's own loop, with
/// the time noted at each decision).
pub(crate) fn run(c: &mut Composer, res: &Resolved, seconds: f64, rec: &mut Recorder) {
    let rate = 48000.0;
    let end = c.seconds() + seconds;
    while c.seconds() < end {
        rec.now_s = c.seconds();
        c.process(res, rec);
        let n = c.until_next().min(((end - c.seconds()) * rate).ceil().max(1.0) as usize).max(1);
        c.advance(n);
    }
}

pub(crate) fn steer(lib: &MusicLibrary, mood: &str, genre: &str) -> Steer {
    let s = lib.genre_index(genre).unwrap() as u16;
    Steer { mood: lib.mood_index(mood).unwrap() as u16, genre_a: s, genre_b: s, blend: 0.0, intensity: 0.3, a4: 432.0 }
}

/// THE PITCH TABLE AT A = 432 Hz: every note of D Lydian from D2 to D6 (the calm mood's scale,
/// the register the score plays in), against values typed here from 432 x 2^((n - 69) / 12) to three
/// decimals; A = 440 is the same function with one number changed. RED first (2026-11-06) with the
/// tuning's default left at 440 (`A4_DEFAULT_HZ = 440.0`): "D2 at the default tuning: 73.416 Hz, the
/// table says 72.081".
#[test]
fn the_pitch_table_at_432_holds_every_scale_note() {
    let table = [
        ("D2", 72.081), ("E2", 80.909), ("F#2", 90.817), ("G#2", 101.938), ("A2", 108.000), ("B2", 121.226), ("C#3", 136.071),
        ("D3", 144.163), ("E3", 161.817), ("F#3", 181.634), ("G#3", 203.877), ("A3", 216.000), ("B3", 242.452), ("C#4", 272.143),
        ("D4", 288.325), ("E4", 323.634), ("F#4", 363.267), ("G#4", 407.754), ("A4", 432.000), ("B4", 484.904), ("C#5", 544.286),
        ("D5", 576.651), ("E5", 647.269), ("F#5", 726.535), ("G#5", 815.507), ("A5", 864.000), ("B5", 969.807), ("C#6", 1088.572),
        ("D6", 1153.302),
    ];
    for (name, hz) in table {
        let midi = parse_note(name).unwrap_or_else(|| panic!("{name} parses"));
        let got = note_hz(midi as f32, crate::tuning::A4_DEFAULT_HZ);
        assert!((got as f64 - hz).abs() < 0.0015, "{name} at the default tuning: {got:.3} Hz, the table says {hz}");
    }
    assert_eq!(Tuning::default(), Tuning::A432, "432 is the default");
    assert_eq!(Tuning::A432.a4_hz(), 432.0);
    // 440 is one number: every note exactly 440/432 higher (31.77 cents).
    for (name, _) in table {
        let m = parse_note(name).unwrap() as f32;
        let cents = 1200.0 * (note_hz(m, 440.0) as f64 / note_hz(m, 432.0) as f64).log2();
        assert!((cents - 31.767).abs() < 0.01, "{name}: {cents} cents");
    }
    assert_eq!(parse_note("A4"), Some(69));
    assert_eq!(parse_note("Bb1"), Some(34));
    assert_eq!(parse_note("C-1"), Some(0));
    assert_eq!(parse_note("H2"), None);
}

/// THE SHIPPED MUSIC DATA PARSES AND RESOLVES (in `just validate-data`): every instrument, mood,
/// genre, the places, the tones, the intensity bands and the hum sources read, every name in them
/// resolves, and the defaults are first. RED first (2026-11-06), before the music's words were in
/// en.json: "music.genre.mothership_calm has no words in en.json".
#[test]
fn the_shipped_music_data_parses() {
    let lib = shipped();
    assert_eq!(lib.moods[0].id, crate::library::DEFAULT_MOOD);
    assert_eq!(lib.genres[0].id, crate::library::DEFAULT_GENRE);
    for id in ["mothership_calm", "menu", "garden_morning", "night_watch", "tension", "action"] {
        assert!(lib.mood_index(id).is_some(), "mood {id}");
    }
    for id in ["steel_pan", "deep_drum", "taiko", "taiko_hit", "brass_stab"] {
        assert!(lib.instrument_index(id).is_some(), "instrument {id}");
    }
    assert!(lib.genre_index("lofi").is_some());
    assert!(!lib.hum.sources.is_empty(), "the hum has its sources");
    assert_eq!(lib.tones.len(), 9);
    Resolved::new(&lib, &SampleLibrary::new()).expect("resolves");
    // Every words key the music data names has English (a genre's name in Settings, an
    // instrument's in the instrument list).
    let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data_dir().join("i18n/en.json")).unwrap()).unwrap();
    let has = |key: &str| {
        let mut v = &en;
        for part in key.split('.') {
            v = &v[part];
        }
        v.as_str().is_some_and(|s| !s.is_empty())
    };
    for k in lib.genres.iter().flat_map(|s| [s.words.clone(), s.about_words.clone()]).chain(lib.moods.iter().map(|m| m.words.clone())).chain(lib.instruments.iter().map(|i| i.words.clone())) {
        assert!(k.is_empty() || has(&k), "{k} has no words in en.json");
    }
}

/// PLACES MAP EVERY ZONE PURPOSE TO A MOOD (rung 3): the six purposes of the ship's zones by day and
/// by night, and the rooms that ask for a garden or a watch. RED first (2026-11-06) with places.ron's
/// agriculture rule removed: "agriculture at 9:00" (left: "mothership_calm", right: "garden_morning").
#[test]
fn places_map_each_zone_purpose_to_a_mood() {
    let lib = shipped();
    let at = |zone: &str, room: &str, hour: f32| lib.places.mood_for(&Place { zone, room_id: room, room_name: room, hour }).to_string();
    for zone in ["residence", "commons", "street", "bay", "agriculture", "corridor"] {
        for hour in [9.0, 14.0, 23.0] {
            let m = at(zone, "", hour);
            assert!(lib.mood_index(&m).is_some(), "{zone} at {hour}: {m} is a mood");
        }
    }
    assert_eq!(at("residence", "", 14.0), "mothership_calm");
    assert_eq!(at("commons", "", 10.0), "mothership_calm");
    assert_eq!(at("agriculture", "", 9.0), "garden_morning", "agriculture at 9:00");
    assert_eq!(at("corridor", "", 14.0), "night_watch");
    assert_eq!(at("residence", "", 23.0), "night_watch");
    assert_eq!(at("residence", "greenhouse", 8.0), "garden_morning");
    assert_eq!(at("", "", 3.0), "night_watch");
    assert_eq!(lib.places.genre_for(&Place { zone: "residence", room_id: "gym", room_name: "gym", hour: 10.0 }), Some("lofi"));
    assert_eq!(lib.places.genre_for(&Place { zone: "residence", room_id: "bedroom", room_name: "bedroom", hour: 10.0 }), None);
}

/// The first phrase that repeats one played within `window` seconds before it: (its index, its
/// time, the earlier one's time).
pub(crate) fn repeated_phrase(rec: &Recorder, window: f64) -> Option<(usize, f64, f64)> {
    // A section RETURNING the first time through its piece's form repeats its tune on purpose
    // (docs/design/music.md section 7c), so it is not counted.
    let phrases: Vec<(u64, f64)> = rec.events.iter().filter_map(|e| if let Event::Phrase { hash, at_s, returning: false, .. } = e { Some((*hash, *at_s)) } else { None }).collect();
    for (i, &(h, t)) in phrases.iter().enumerate() {
        if let Some(&(_, t2)) = phrases[..i].iter().find(|&&(h2, t2)| h == h2 && t - t2 < window) {
            return Some((i, t, t2));
        }
    }
    None
}

/// The first two-minute stretch (from a chord change: its chord and motif events, times relative to
/// it) that repeats an earlier one: (its start, the earlier start), and how many were checked.
pub(crate) fn repeated_stretch(rec: &Recorder, total_s: f64) -> (Option<(f64, f64)>, usize) {
    let mut evs: Vec<(f64, u64)> = Vec::new();
    for e in &rec.events {
        match e {
            Event::Chord { chord, pcs, at_s, beats } => evs.push((*at_s, (*chord as u64) << 32 | (*pcs as u64) << 8 | (*beats as u64))),
            Event::Motif { role, first_note, at_s } => evs.push((*at_s, 1 << 60 | (*role as u64) << 16 | (*first_note as u64 & 0xff))),
            _ => {}
        }
    }
    let starts: Vec<usize> = (0..evs.len()).filter(|&i| evs[i].1 >> 60 == 0 && evs[i].0 + 120.0 <= total_s).collect();
    let mut seen = std::collections::HashMap::new();
    for &s in &starts {
        let t0 = evs[s].0;
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &(t, v) in evs[s..].iter().take_while(|(t, _)| *t < t0 + 120.0) {
            for x in [((t - t0) * 10.0).round() as u64, v] {
                h = (h ^ x).wrapping_mul(0x100_0000_01b3);
            }
        }
        if let Some(prev) = seen.insert(h, t0) {
            return (Some((t0, prev)), starts.len());
        }
    }
    (None, starts.len())
}

/// THE SCORE NEVER REPEATS A PHRASE WITHIN TEN MINUTES, and an hour holds no repeated two-minute
/// stretch: the composer runs an hour (in milliseconds, no sound); every phrase's plan fingerprint
/// (what will sound: key, chords and their lengths, motifs and their variations, choir, pulse, the
/// playing drum parts' patterns) differs from every one in the ten minutes before it, and the chord
/// and motif events of every two-minute window starting at a chord change hash differently from
/// every other window's. The detectors are proven able to fail (a positive control): a fixture mood
/// of two chords and a pad and nothing else has one possible phrase, so it must be caught repeating.
/// RED first (2026-11-06) with that control asserting no repeat: "the fixture: phrase 1 at 32.0 s
/// repeats the one from 0.0 s".
#[test]
fn the_score_never_repeats_a_phrase_within_ten_minutes() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    for (mood, seed) in [("mothership_calm", 1u64), ("night_watch", 2), ("garden_morning", 3)] {
        let mut c = Composer::new(48000.0, seed, steer(&lib, mood, "mothership_calm"));
        let mut rec = Recorder::default();
        run(&mut c, &res, 3600.0, &mut rec);
        let phrases = rec.events.iter().filter(|e| matches!(e, Event::Phrase { .. })).count();
        assert!(phrases > 40, "{mood}: {phrases} phrases in an hour");
        if let Some((i, t, t2)) = repeated_phrase(&rec, 600.0) {
            panic!("{mood}: phrase {i} at {t:.1} s repeats the one from {t2:.1} s");
        }
        let (stretch, checked) = repeated_stretch(&rec, 3600.0);
        if let Some((t0, prev)) = stretch {
            panic!("{mood}: the two minutes from {t0:.1} s repeat the two minutes from {prev:.1} s");
        }
        assert!(checked > 100, "{mood}: {checked} windows checked");
    }
    // The positive control: a mood that can only repeat is caught repeating.
    let mut lib2 = shipped();
    let fixture: crate::mood::Mood = ron::from_str(
        r#"Mood(id: "two_chords", keys: [(root: "D", mode: Ionian)],
            chords: [(name: "A", root: 0, notes: [0, 4, 7]), (name: "B", root: 5, notes: [0, 4, 7])],
            moves: [("A", "B", 1.0), ("B", "A", 1.0)], cadences: ["A", "B"], start: "A",
            phrase_bars: [8], chord_bars: (2, 2), tempo_bpm: (60.0, 60.0),
            roles: [(role: Pad, instrument: "soft_saw_pad", low: "D3", high: "D5", voices: 3)])"#,
    )
    .unwrap();
    lib2.moods.push(fixture);
    let res2 = Resolved::new(&lib2, &SampleLibrary::new()).unwrap();
    let mut c = Composer::new(48000.0, 1, steer(&lib2, "two_chords", "mothership_calm"));
    let mut rec = Recorder::default();
    run(&mut c, &res2, 600.0, &mut rec);
    assert!(repeated_phrase(&rec, 600.0).is_some(), "the fixture's repeats were not caught");
    assert!(repeated_stretch(&rec, 600.0).0.is_some(), "the fixture's repeated stretches were not caught");
}

/// VOICE LEADING MOVES NO VOICE MORE THAN A FOURTH: an hour in which the place's mood changes every
/// five minutes (calm, night, garden, menu); every pad and choir voicing against the one before it,
/// lowest voice to lowest, never more than 5 semitones. RED first (2026-11-06) with `voice_lead`
/// spreading every chord over the register afresh (no leading): "role 3 voice 0 moved 8 semitones
/// (59 to 67) at 15.1 s" (role 3 is the choir).
#[test]
fn voice_leading_moves_no_voice_more_than_a_fourth() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let mut c = Composer::new(48000.0, 7, steer(&lib, "mothership_calm", "mothership_calm"));
    let mut rec = Recorder::default();
    for (i, mood) in ["mothership_calm", "night_watch", "garden_morning", "menu"].iter().cycle().take(12).enumerate() {
        let _ = i;
        c.steer.mood = lib.mood_index(mood).unwrap() as u16;
        run(&mut c, &res, 300.0, &mut rec);
    }
    let mut last: [Option<([i32; 8], u8)>; 8] = [None; 8];
    let mut checked = 0;
    for e in &rec.events {
        if let Event::Voicing { role, notes, n, at_s } = e {
            if let Some((prev, pn)) = last[*role as usize] {
                if pn == *n {
                    for k in 0..*n as usize {
                        let d = (notes[k] - prev[k]).abs();
                        assert!(d <= 5, "role {role} voice {k} moved {d} semitones ({} to {}) at {at_s:.1} s", prev[k], notes[k]);
                    }
                    checked += 1;
                }
            }
            last[*role as usize] = Some((*notes, *n));
        }
    }
    assert!(checked > 300, "{checked} voicings checked");
}

/// THE GENRE LAYER CHANGES TEMPO, GROOVE AND PALETTE BY DATA ALONE: a test genre written here (no
/// code) laid over the shipped calm mood plays at its 120 bpm, swings its off-beats to 0.66 of the
/// beat, plays its drum pattern, and moves the pad to its palette's instrument and off the choir;
/// the default genre leaves the mood as written (56 to 64 bpm, no drums, the saw pad). RED first
/// (2026-11-06) with `resolve_eff` taking the tempo from the mood alone: "the test genre's tempo:
/// 59.8 bpm, wanted 120".
#[test]
fn a_genre_changes_tempo_groove_and_palette_by_data_alone() {
    let mut lib = shipped();
    let test_genre: crate::genre::Genre = ron::from_str(
        r#"Genre(
            id: "test_swing",
            name: "Test swing",
            tempo_bpm: Some((120.0, 120.0)),
            beat_felt: Some(true),
            groove: Some((swing: 0.66, subdivision: 2)),
            drums: [(instrument: "lofi_hat", note: "D5", patterns: ["xxxxxxxx"])],
            palette: [(role: Pad, instrument: "electric_piano")],
            roles_off: [Choir],
        )"#,
    )
    .expect("the test genre parses");
    lib.genres.push(test_genre);
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let hat = lib.instrument_index("lofi_hat").unwrap() as u16;
    let ep = lib.instrument_index("electric_piano").unwrap() as u16;
    let saw = lib.instrument_index("soft_saw_pad").unwrap() as u16;
    let choir = lib.instrument_index("choir_ah").unwrap() as u16;

    let mut c = Composer::new(48000.0, 11, steer(&lib, "mothership_calm", "test_swing"));
    let mut rec = Recorder::default();
    run(&mut c, &res, 60.0, &mut rec);
    assert!((c.tempo() - 120.0).abs() < 1e-3, "the test genre's tempo: {:.1} bpm, wanted 120", c.tempo());
    let hats: Vec<f64> = rec.notes.iter().filter(|(_, n)| n.inst == hat).map(|(t, _)| *t).collect();
    assert!(hats.len() > 100, "{} hat hits", hats.len());
    // Beat 0.5 s at 120 bpm: on-beats every 0.5 s, off-beats 0.33 s after them (0.66 of the beat).
    let beat = 0.5;
    let offs: Vec<f64> = hats.windows(2).map(|w| w[1] - w[0]).collect();
    let (short, long) = (offs.iter().copied().fold(f64::MAX, f64::min), offs.iter().copied().fold(0.0, f64::max));
    assert!((long - 0.66 * beat).abs() < 0.002 && (short - 0.34 * beat).abs() < 0.002, "swung gaps {short:.3} s and {long:.3} s, wanted {:.3} and {:.3}", 0.34 * beat, 0.66 * beat);
    let pads: Vec<u16> = rec.notes.iter().filter(|(_, n)| n.role == RoleKind::Pad.index() as u8).map(|(_, n)| n.inst).collect();
    assert!(!pads.is_empty() && pads.iter().all(|&i| i == ep), "the pad plays the palette's electric piano");
    assert!(!rec.notes.iter().any(|(_, n)| n.inst == choir), "the choir is off");

    let mut c = Composer::new(48000.0, 11, steer(&lib, "mothership_calm", "mothership_calm"));
    let mut rec = Recorder::default();
    run(&mut c, &res, 60.0, &mut rec);
    assert!((56.0..=64.0).contains(&c.tempo()), "the default genre keeps the mood's tempo: {}", c.tempo());
    assert!(!rec.notes.iter().any(|(_, n)| n.inst == hat), "no drums in calm");
    assert!(rec.notes.iter().filter(|(_, n)| n.role == RoleKind::Pad.index() as u8).all(|(_, n)| n.inst == saw));
}

/// The notes per minute and the instruments heard in a stretch of a run.
fn density(rec: &Recorder, from: f64, to: f64) -> (f64, std::collections::BTreeSet<u16>) {
    let n: Vec<&NoteOn> = rec.notes.iter().filter(|(t, _)| *t >= from && *t < to).map(|(_, n)| n).collect();
    (n.len() as f64 * 60.0 / (to - from), n.iter().map(|n| n.inst).collect())
}

/// INTENSITY DRIVES THE SCORE BY DATA: tempo, density and palette (operator, 2026-11-06, the meteor
/// storm). Calm at 0, tension at 0.5 (intensity.ron's band from 0.35), action at 0.9: the tempo is
/// the calm mood's, then tension's 84 to 100, then action's past 118; the notes a minute more than
/// double; the palette moves to the taiko, the big hit and the brass stabs; the taiko's sixteenths
/// (min_intensity 0.85) play at 0.9 and not at 0.75; the rise is marked by the stinger's big hit
/// within a beat and the band changes on a bar line; back at 0 it settles to the calm mood. RED first
/// (2026-11-06) with `check_intensity` returning at once (bands read but never followed):
/// "assertion failed: tension" (left: 0, right: 1).
#[test]
fn intensity_drives_tempo_density_and_palette_by_data() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let id = |s: &str| lib.instrument_index(s).unwrap() as u16;
    let mut c = Composer::new(48000.0, 5, steer(&lib, "mothership_calm", "mothership_calm"));
    let mut rec = Recorder::default();
    c.steer.intensity = 0.0;
    run(&mut c, &res, 60.0, &mut rec);
    let calm_tempo = c.tempo();
    assert!((56.0..=64.0).contains(&calm_tempo), "calm tempo {calm_tempo}");
    assert_eq!(c.band(), 0);
    let (calm_rate, calm_set) = density(&rec, 10.0, 60.0);

    c.steer.intensity = 0.5;
    let rise_at = c.seconds();
    run(&mut c, &res, 40.0, &mut rec);
    assert_eq!(c.band(), 1, "tension");
    assert_eq!(c.mood(), lib.mood_index("tension").unwrap());
    assert!((84.0..=100.0).contains(&c.tempo()), "tension's tempo: {:.1} bpm", c.tempo());
    // The stinger's big hit within one calm beat of the rise.
    let hit = rec.notes.iter().find(|(t, n)| *t >= rise_at && n.inst == id("taiko_hit")).map(|(t, _)| *t).expect("the stinger's hit");
    assert!(hit - rise_at <= 60.0 / calm_tempo as f64 + 1e-3, "the stinger came {:.2} s after the rise", hit - rise_at);
    let band_at = rec.events.iter().find_map(|e| if let Event::Band { band: 1, at_s } = e { Some(*at_s) } else { None }).expect("the band change");
    assert!(band_at > rise_at && band_at - rise_at <= 2.0 * 4.0 * 60.0 / calm_tempo as f64 + 0.01, "the band changed {:.2} s after the rise, within two bars", band_at - rise_at);

    c.steer.intensity = 0.75;
    run(&mut c, &res, 30.0, &mut rec);
    assert_eq!(c.mood(), lib.mood_index("action").unwrap(), "action at 0.75");
    let t75 = c.seconds();
    run(&mut c, &res, 20.0, &mut rec);
    let sixteenths = |rec: &Recorder, from: f64, to: f64| rec.notes.iter().filter(|(t, n)| *t >= from && *t < to && n.inst == id("taiko") && (n.midi - 45.0).abs() < 0.01).count();
    assert_eq!(sixteenths(&rec, t75, c.seconds()), 0, "the taiko's sixteenths wait for 0.85");

    c.steer.intensity = 0.9;
    run(&mut c, &res, 40.0, &mut rec);
    assert!(c.tempo() >= 118.0, "action's tempo at 0.9: {:.1}", c.tempo());
    let end = c.seconds();
    let (action_rate, action_set) = density(&rec, end - 30.0, end);
    assert!(sixteenths(&rec, end - 30.0, end) > 100, "the taiko's sixteenths at 0.9");
    assert!(action_rate > 2.0 * calm_rate, "notes a minute: calm {calm_rate:.0}, action {action_rate:.0}");
    for inst in ["taiko", "taiko_hit", "brass_stab", "pulse_bass"] {
        assert!(action_set.contains(&id(inst)) && !calm_set.contains(&id(inst)), "{inst} only in action");
    }

    c.steer.intensity = 0.0;
    run(&mut c, &res, 60.0, &mut rec);
    assert_eq!(c.band(), 0, "settled");
    assert_eq!(c.mood(), lib.mood_index("mothership_calm").unwrap());
    assert!((56.0..=64.0).contains(&c.tempo()));
}

/// The same with a FIXTURE intensity map written here: data alone decides which mood a band plays,
/// so a band can send the score to the garden. RED first (2026-11-06) with `Resolved::band_mood`
/// returning None: "band 1 plays mothership_calm" (left: "mothership_calm", right: "garden_morning").
#[test]
fn an_intensity_band_plays_the_mood_its_data_names() {
    let mut lib = shipped();
    lib.intensity = ron::from_str(r#"IntensityMap(bands: [(from: 0.0), (from: 0.5, mood: Some("garden_morning"))], stinger: [], riser: None)"#).unwrap();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let mut c = Composer::new(48000.0, 9, steer(&lib, "mothership_calm", "mothership_calm"));
    let mut rec = Recorder::default();
    c.steer.intensity = 0.0;
    run(&mut c, &res, 20.0, &mut rec);
    c.steer.intensity = 0.8;
    run(&mut c, &res, 30.0, &mut rec);
    assert_eq!(lib.moods[c.mood()].id, "garden_morning", "band 1 plays {}", lib.moods[c.mood()].id);
}

/// THE BANDS' HYSTERESIS: an intensity hovering at an edge does not flip the band. RED first
/// (2026-11-06) with the rise's hysteresis taken out: "just past the edge is not enough" (left: 1,
/// right: 0).
#[test]
fn a_band_edge_has_hysteresis() {
    use crate::intensity::band_for;
    let f = [0.0, 0.35, 0.7];
    assert_eq!(band_for(&f, 0, 0.36, 0.08), 0, "just past the edge is not enough");
    assert_eq!(band_for(&f, 0, 0.40, 0.08), 1);
    assert_eq!(band_for(&f, 1, 0.33, 0.08), 1, "just under is not enough to fall");
    assert_eq!(band_for(&f, 1, 0.30, 0.08), 0);
    assert_eq!(band_for(&f, 0, 0.95, 0.08), 2, "straight to the top");
}

/// EVERY GENRE HAS ITS WORDS AND ITS INSTRUMENT ROLES (2026-11-12, the genre selector; the operator:
/// "for the procedural music, let's add a genre selector"): the genres load from their files
/// (data/music/genres/*.ron, the default first, electro swing among them); every genre has its name
/// and its one sentence in English (or plain words, for a community file); every genre but the
/// default (which changes nothing, so it plays the mood's own roles) names the instruments its roles
/// play; every instrument a genre names (drums, palette, added roles) is an instrument of the score
/// with its words, and every part it names is a part of the instrument library (parts.ron); and
/// electro swing's roles are the ones its design names: a walking upright bass, gypsy-jazz guitar
/// comping, brass stabs, clarinet calls answered by brass, and the house kit (a four-on-the-floor
/// kick, a clap, an open hi-hat on every off-beat). RED first (2026-11-12), before
/// data/music/genres/electro_swing.ron existed: "no genre electro_swing in [\"mothership_calm\",
/// \"lofi\"]".
#[test]
fn every_genre_has_its_words_and_instrument_roles() {
    let lib = shipped();
    let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data_dir().join("i18n/en.json")).unwrap()).unwrap();
    let has = |key: &str| {
        let mut v = &en;
        for part in key.split('.') {
            v = &v[part];
        }
        v.as_str().is_some_and(|s| !s.is_empty())
    };
    let parts = std::fs::read_to_string(data_dir().join("music/parts.ron")).unwrap();
    let ids: Vec<&str> = lib.genres.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids[0], crate::library::DEFAULT_GENRE, "the default genre is first");
    let es = lib.genres.iter().find(|g| g.id == "electro_swing").unwrap_or_else(|| panic!("no genre electro_swing in {ids:?}"));
    for g in &lib.genres {
        assert!((!g.words.is_empty() && has(&g.words)) || !g.name.is_empty(), "{}: its name has no words", g.id);
        assert!((!g.about_words.is_empty() && has(&g.about_words)) || !g.about.is_empty(), "{}: its one sentence has no words", g.id);
        let mut named: Vec<(&str, Option<&str>)> = g.drums.iter().map(|d| (d.instrument.as_str(), None)).collect();
        named.extend(g.palette.iter().map(|p| (p.instrument.as_str(), p.part.as_deref())));
        named.extend(g.roles_add.iter().map(|r| (r.instrument.as_str(), r.part.as_deref())));
        if g.id != crate::library::DEFAULT_GENRE {
            assert!(!g.palette.is_empty() || !g.roles_add.is_empty(), "{}: names no instrument for any role", g.id);
        }
        for (inst, part) in named {
            let i = lib.instrument_index(inst).unwrap_or_else(|| panic!("{}: {inst:?} is not an instrument of the score", g.id));
            assert!(has(&lib.instruments[i].words), "{}: the instrument {inst} has no words", g.id);
            if let Some(p) = part {
                assert!(parts.contains(&format!("(id: \"{p}\"")), "{}: the part {p} is not in parts.ron", g.id);
            }
        }
    }
    let role = |r: RoleKind| es.roles_add.iter().find(|a| a.role == r).unwrap_or_else(|| panic!("electro swing adds its {r:?}"));
    assert_eq!(role(RoleKind::Bass).part.as_deref(), Some("double_bass_pizzicato"), "an upright bass");
    assert_eq!(role(RoleKind::Pad).part.as_deref(), Some("acoustic_guitar"), "the steel-string guitar comps (the Martin, 2026-11-13)");
    assert_eq!(role(RoleKind::Choir).part.as_deref(), Some("trumpet"), "brass stabs");
    assert_eq!(role(RoleKind::Melody).part.as_deref(), Some("clarinet"), "clarinet lines");
    assert_eq!(role(RoleKind::Bells).part.as_deref(), Some("trombone"), "brass answers");
    assert!(es.call_and_response, "call and response");
    assert!(!es.stab_patterns.is_empty(), "the stabs have their patterns");
    assert!(es.bass_patterns.iter().any(|p| p.contains('w')), "the bass walks");
    let drum = |id: &str| es.drums.iter().find(|d| d.instrument == id).unwrap_or_else(|| panic!("electro swing's {id}"));
    assert!(drum("house_kick").patterns.iter().all(|p| p.starts_with("x.x.x.x")), "four on the floor");
    assert!(drum("open_hat").patterns.iter().all(|p| p == ".x.x.x.x"), "the open hat on every off-beat");
    drum("clap");
}

/// DANGER RISES AND SETTLES IN EVERY GENRE (2026-11-12; the brief: "The score's own math (danger
/// rising and settling) keeps working in every genre"): for each shipped genre, calm at 0; at 0.5 the
/// tension band plays tension AS WRITTEN (its own D Phrygian and its 84 to 100 bpm, whatever the
/// genre's harmony); at 0.9 action; back at 0 the score settles into the calm mood with the genre's
/// own tempo and harmony again (electro swing: 128 bpm in its harmonic minor keys). RED first
/// (2026-11-12) with a genre's progressions let into the danger moods (start_phrase's `calm` forced
/// true): "electro_swing: tension plays its own harmony: Phrygian, got HarmonicMinor".
#[test]
fn danger_rises_and_settles_in_every_genre() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let calm_mood = lib.mood_index("mothership_calm").unwrap();
    for g in &lib.genres {
        let mut c = Composer::new(48000.0, 7, steer(&lib, "mothership_calm", &g.id));
        let mut rec = Recorder::default();
        c.steer.intensity = 0.0;
        run(&mut c, &res, 30.0, &mut rec);
        assert_eq!(c.band(), 0, "{}: calm", g.id);
        c.steer.intensity = 0.5;
        run(&mut c, &res, 30.0, &mut rec);
        assert_eq!(c.band(), 1, "{}: tension", g.id);
        assert_eq!(c.mood(), lib.mood_index("tension").unwrap(), "{}: the tension mood", g.id);
        assert_eq!(c.mode(), crate::tuning::Mode::Phrygian, "{}: tension plays its own harmony: Phrygian, got {:?}", g.id, c.mode());
        assert!((84.0..=100.0).contains(&c.tempo()), "{}: tension's tempo {:.1}", g.id, c.tempo());
        c.steer.intensity = 0.9;
        run(&mut c, &res, 30.0, &mut rec);
        assert_eq!(c.band(), 2, "{}: action", g.id);
        assert_eq!(c.mood(), lib.mood_index("action").unwrap(), "{}: the action mood", g.id);
        c.steer.intensity = 0.0;
        run(&mut c, &res, 40.0, &mut rec);
        assert_eq!(c.band(), 0, "{}: settled", g.id);
        assert_eq!(c.mood(), calm_mood, "{}: back in the calm mood", g.id);
        let (lo, hi) = g.tempo_bpm.unwrap_or(lib.moods[calm_mood].tempo_bpm);
        assert!(c.tempo() >= lo - 1e-3 && c.tempo() <= hi + 1e-3, "{}: back at its tempo, {:.1} bpm", g.id, c.tempo());
        let modes: Vec<crate::tuning::Mode> = match &g.progressions {
            Some(p) => p.keys.iter().map(|k| k.mode).collect(),
            None => lib.moods[calm_mood].keys.iter().map(|k| k.mode).collect(),
        };
        assert!(modes.contains(&c.mode()), "{}: back in its own harmony, got {:?}", g.id, c.mode());
    }
}
