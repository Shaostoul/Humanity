// The Music page's crate half (docs/design/music-page.md): favourites saved and loaded, share codes,
// the export, the piano roll and the chord symbols, each against an independent reading.

use crate::composer::{Composer, Event, Sink};
use crate::favourite::{from_share_code, share_code, Favourite, Favourites, SavedTune, ShareError};
use crate::render::{default_fade_s, export_favourite, write_wav};
use crate::sheet::{chord_symbol, transcribe};
use crate::tests::shipped;
use crate::tune::Tune;
use crate::voice::NoteOn;
use crate::{MusicContext, MusicLibrary, Resolved, SampleLibrary, ScoreEngine};

/// A folder of this test's own, removed when done.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let p = std::env::temp_dir().join(format!("hos-music-page-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn res(lib: &MusicLibrary) -> Resolved {
    Resolved::new(lib, &SampleLibrary::new()).expect("resolve")
}

/// A favourite with every part of a tuning changed: electro swing (a genre with a form and lead
/// voices), its clarinet's place taken by the second melody lead the genre offers.
fn tuned_favourite(lib: &MusicLibrary) -> Favourite {
    tuned(lib).0
}

/// The same favourite and the tuning it was made from (as the page holds it).
fn tuned(lib: &MusicLibrary) -> (Favourite, Tune) {
    let g = lib.genre_index("electro_swing").expect("electro swing ships");
    let melody = crate::mood::RoleKind::Melody.index();
    let lead = lib.genres[g].leads.iter().enumerate().filter(|(_, e)| e.role.index() == melody).map(|(i, _)| i).nth(1).expect("electro swing offers several melody leads");
    let mut t = Tune { transpose: 3, tempo_bpm: 120.0, swing: 0.6, ..Tune::default() };
    t.lead[melody] = lead as u16;
    t.level_db[1] = -3.0;
    t.mute = 1 << 7;
    (Favourite { id: 0, name: "Car, Tuesday".into(), genre: "electro_swing".into(), mood: "mothership_calm".into(), seed: 4242, tune: SavedTune::from_tune(&t, lib, g) }, t)
}

/// A FAVOURITE SAVED AND LOADED PLAYS THE SAME PIECE: the file read back is the favourite, and the
/// piece it composes is the same note for note (the transcription's event hash over a minute), while
/// another seed, or the same seed untuned, composes another.
///
/// Seen RED (2026-11-13), with `SavedTune::from_tune` not writing the lead voices (the `leads` map
/// left empty): the tuning read back had `lead: [65535, ...]` where the page held melody = 1 ("the
/// tuning the page held comes back whole"); and with that line removed, the hashes differed
/// (392390909177007983 against 703066606960335530: the piece's own lead in place of the chosen one).
/// (A first version compared two favourites both made by `from_tune` and stayed green under that
/// break: the check now starts from the `Tune` the page holds.)
#[test]
fn a_favourite_saved_and_loaded_plays_the_same_piece() {
    let lib = shipped();
    let r = res(&lib);
    let (fav, tune) = tuned(&lib);
    let mut favs = Favourites::default();
    let id = favs.add(fav);
    favs.assign("tension", Some(id));
    let dir = Scratch::new("favs");
    let path = dir.0.join("music").join("favourites.json");
    favs.save(&path).expect("saved");
    let back = Favourites::load(&path).expect("loaded");
    assert_eq!(back, favs, "the file reads back as it was written");
    let kept = favs.get(id).unwrap();
    let loaded = back.get(id).unwrap();
    let g = lib.genre_index("electro_swing").unwrap();
    assert_eq!(loaded.tune.to_tune(&lib, g), tune, "the tuning the page held comes back whole");
    let hash = |f: &Favourite| transcribe(&r, f.seed, &f.context(&lib, 432.0).unwrap(), 48000.0, 60.0).hash;
    // What the page played before it was kept: its context built from the tuning it held.
    let played = MusicContext { tune, ..kept.context(&lib, 432.0).unwrap() };
    assert_eq!(transcribe(&r, kept.seed, &played, 48000.0, 60.0).hash, hash(loaded), "the loaded favourite composes the piece the page played");
    assert_eq!(hash(kept), hash(loaded));
    let other_seed = Favourite { seed: 4243, ..kept.clone() };
    assert_ne!(hash(kept), hash(&other_seed), "another seed is another piece (the hash can tell)");
    let untuned = Favourite { tune: SavedTune::default(), ..kept.clone() };
    assert_ne!(hash(kept), hash(&untuned), "the tuning is part of what is composed");
    assert_eq!(back.for_mood("tension").map(|f| f.id), Some(id), "the mood it is the music for is kept");
}

/// A file mended by hand and broken is moved aside, never written over.
#[test]
fn an_unreadable_favourites_file_is_moved_aside() {
    let dir = Scratch::new("aside");
    let path = dir.0.join("favourites.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(Favourites::load(&path).is_err());
    Favourites::default().save(&path).expect("saved");
    assert_eq!(std::fs::read_to_string(path.with_extension("unreadable.json")).unwrap(), "{ not json");
    assert_eq!(Favourites::load(&path).unwrap(), Favourites::default());
}

/// A SHARE CODE ROUND-TRIPS: a tuned favourite as a code and back is the same favourite (its id
/// stays behind); a mistyped letter is refused by the checksum, not played as something else; and
/// the confusable letters read as Crockford says (lower case, O for 0).
///
/// Seen RED (2026-11-13): with the decoder reading the swing byte as `swing / 100` (the encoder
/// writes `swing * 200`), the round trip failed (`swing: Some(1.2)` against `Some(0.6)`).
#[test]
fn a_share_code_round_trips_and_a_mistyped_one_is_refused() {
    let lib = shipped();
    let fav = tuned_favourite(&lib);
    let code = share_code(&fav);
    assert!(code.starts_with("HOSM1-"), "{code}");
    // About 120 characters with every part of the tuning changed and a name; an untuned favourite
    // with a short name is about 90.
    assert!(code.len() < 130, "a code a person can paste: {} characters", code.len());
    let big = Favourite { seed: u64::MAX, ..fav.clone() };
    assert_eq!(from_share_code(&share_code(&big)).expect("the largest seed reads"), big);
    assert_eq!(from_share_code(&code).expect("reads"), fav);
    assert_eq!(from_share_code(&code.to_lowercase()).expect("case is ignored"), fav);
    assert_eq!(from_share_code(&code.replace('0', "O")).expect("O reads as zero"), fav);
    // One letter changed anywhere in the body is refused.
    let body_start = "HOSM1-".len();
    for at in [body_start, body_start + 7, code.len() / 2, code.len() - 1] {
        let mut chars: Vec<char> = code.chars().collect();
        chars[at] = if chars[at] == 'Q' { 'R' } else { 'Q' };
        let typo: String = chars.into_iter().collect();
        assert!(matches!(from_share_code(&typo), Err(ShareError::Mistyped) | Err(ShareError::Garbled)), "a typo at {at} is refused");
    }
    assert_eq!(from_share_code("hello"), Err(ShareError::NotACode));
    assert_eq!(from_share_code("HOSM1-UUUU"), Err(ShareError::Garbled), "U is not in the alphabet");
}

/// THE EXPORTED WAV OF SEED S IS THE OFFLINE RENDER OF SEED S, byte for byte: the page's export
/// (`render::export_favourite`) against `just music-render`'s own loop (examples/music_render.rs:
/// the context from the mood and genre, set every 128-frame block, the fade), both written as WAV
/// and hashed.
///
/// Seen RED (2026-11-13): with the export's seed off by one (`fav.seed + 1` in
/// `export_favourite`), the hashes differed (5079948750349539708 against 18359981080809876721: two
/// pieces); with it put back, equal.
#[test]
fn the_exported_wav_of_a_seed_is_the_offline_render_of_it() {
    let lib = shipped();
    let seconds = 12.0;
    let fav = Favourite { id: 0, name: "Seed 777".into(), genre: "lofi".into(), mood: "mothership_calm".into(), seed: 777, tune: SavedTune::default() };
    let dir = Scratch::new("export");
    let exported = dir.0.join("export.wav");
    export_favourite(res(&lib), &lib, &fav, 432.0, seconds, &exported).expect("exported");
    // music-render's loop, written out here as the example runs it.
    let (mood, genre) = (lib.mood_index("mothership_calm").unwrap() as u16, lib.genre_index("lofi").unwrap() as u16);
    let ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood, genre_a: genre, genre_b: genre, blend: 0.0, intensity: 0.3, duck_db: 0.0, ..Default::default() };
    let mut eng = ScoreEngine::new(res(&lib), 48000.0, 777, ctx);
    let total = (seconds * 48000.0) as usize;
    let mut frames = vec![[0.0f32; 2]; total];
    let mut i = 0;
    while i < total {
        let n = 128.min(total - i);
        eng.set_context(ctx);
        eng.render(&mut frames[i..i + n]);
        i += n;
    }
    crate::render::fade_out(&mut frames, 48000.0, default_fade_s(seconds));
    let offline = dir.0.join("offline.wav");
    write_wav(&offline, &frames, 48000).unwrap();
    let a = std::fs::read(&exported).unwrap();
    let b = std::fs::read(&offline).unwrap();
    assert_eq!(a.len(), 44 + total * 4, "12 s of 16-bit stereo at 48 kHz");
    let fnv = |d: &[u8]| d.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3));
    assert_eq!(fnv(&a), fnv(&b), "the export is the offline render, byte for byte");
    assert!(a[44..].iter().any(|&x| x != 0), "and it is not silence");
}

/// A recorder of the composer's own notes and bar lines, stamped with the composer's clock: the
/// independent reading the piano roll is checked against.
#[derive(Default)]
struct Raw {
    now: f64,
    notes: Vec<(f64, NoteOn)>,
}

impl Sink for Raw {
    fn note_on(&mut self, n: NoteOn) {
        self.notes.push((self.now, n));
    }
    fn note_off(&mut self, _tag: u32) {}
    fn event(&mut self, _e: Event) {}
}

/// THE PIANO ROLL'S NOTES ARE THE PIECE'S EVENTS FOR A BAR: every note the roll draws in bar 6 of an
/// electro swing piece (its pitch, its part, where in the bar it starts) is a note the composer
/// started in that bar, and every note the composer started there is drawn, read from a plain dry
/// run of the composer (its own clock and bar counter, not the sheet's loop).
///
/// Seen RED (2026-11-13): with `Sheet::bar_notes` keeping notes up to AND AT the bar's end
/// (`at_s <= b`, an off-by-one at the bar line), the roll drew bar 7's downbeat in bar 6 (an extra
/// `(0, 5200, 400)`, the pad's note on beat 5 of a four-beat bar) and the lists differed.
#[test]
fn the_piano_rolls_notes_are_the_pieces_events_for_a_bar() {
    let lib = shipped();
    let r = res(&lib);
    let fav = Favourite { id: 0, name: String::new(), genre: "electro_swing".into(), mood: "mothership_calm".into(), seed: 9, tune: SavedTune::default() };
    let ctx = fav.context(&lib, 432.0).unwrap();
    let sheet = transcribe(&r, fav.seed, &ctx, 48000.0, 30.0);
    let bar = 6;
    let roll = sheet.bar_notes(bar);
    assert!(roll.len() > 4, "a bar of electro swing has notes: {}", roll.len());
    // The independent reading.
    let mut comp = Composer::new(48000.0, fav.seed, crate::score::steer_of(&ctx));
    let mut raw = Raw::default();
    let mut bars: Vec<f64> = Vec::new();
    while comp.seconds() < 30.0 {
        raw.now = comp.seconds();
        comp.process(&r, &mut raw);
        if comp.bars() as usize > bars.len() {
            bars.push(comp.seconds());
        }
        let n = comp.until_next().max(1);
        comp.advance(n);
    }
    let (a, b) = (bars[bar], bars[bar + 1]);
    let beat = (b - a) / comp.beats_per_bar() as f64;
    let key = |midi: f32, role: u8, start: f64| (role, (midi * 100.0).round() as i64, (start * 100.0).round() as i64);
    let mut want: Vec<_> = raw.notes.iter().filter(|(t, _)| *t >= a - 1e-6 && *t < b - 1e-6).map(|(t, n)| key(n.midi, n.role, (t - a) / beat)).collect();
    let mut got: Vec<_> = roll.iter().map(|n| key(n.midi, n.role, n.start_beat as f64)).collect();
    want.sort();
    got.sort();
    assert_eq!(got, want, "the roll's notes for bar {bar} are the composer's");
    // And the bar has its chord symbol and its section letter.
    assert!(sheet.chord_at(a + 1e-6).is_some(), "a chord sounds in the bar");
    assert!(sheet.section_at(a + 1e-6).is_some_and(|s| s.letter >= 1), "electro swing plays a form: the bar has a section letter");
}

/// Chord symbols as a lead sheet writes them, spelled in the key.
#[test]
fn chord_symbols_are_named_from_their_notes() {
    let mask = |pcs: &[i32]| pcs.iter().fold(0u16, |m, &p| m | 1 << p.rem_euclid(12));
    assert_eq!(chord_symbol(2, mask(&[2, 5, 9, 11]), true), "Dm6");
    assert_eq!(chord_symbol(9, mask(&[9, 1, 4, 7]), true), "A7");
    assert_eq!(chord_symbol(10, mask(&[10, 2, 5, 8]), true), "Bb7");
    assert_eq!(chord_symbol(1, mask(&[1, 4, 7, 10]), false), "C#dim7");
    assert_eq!(chord_symbol(4, mask(&[4, 7, 10, 2]), true), "Em7b5");
    assert_eq!(chord_symbol(0, mask(&[0, 4, 7, 11]), false), "Cmaj7");
    assert!(crate::sheet::key_uses_flats(2, crate::tuning::Mode::HarmonicMinor), "D minor is spelled with B flat");
    assert!(!crate::sheet::key_uses_flats(2, crate::tuning::Mode::Ionian), "D major with F sharp");
}

/// A MUTE NEVER CHANGES WHAT IS COMPOSED: the same seed with the melody muted composes the same notes
/// (the sheet's hash), yet sounds different, and a muted role's meter stays at zero.
#[test]
fn a_mute_silences_a_part_without_changing_the_piece() {
    let lib = shipped();
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let melody = crate::mood::RoleKind::Melody.index();
    let ctx = MusicContext { mood, genre_a: g, genre_b: g, analyse: true, ..Default::default() };
    let muted = MusicContext { tune: Tune { mute: 1 << melody, ..Tune::default() }, ..ctx };
    let r = res(&lib);
    assert_eq!(transcribe(&r, 5, &ctx, 48000.0, 30.0).hash, transcribe(&r, 5, &muted, 48000.0, 30.0).hash);
    let render = |c: MusicContext| {
        let mut e = ScoreEngine::new(res(&lib), 48000.0, 5, c);
        let mut buf = vec![[0.0f32; 2]; 128];
        let mut sum = 0.0f64;
        let mut melody_meter = 0.0f32;
        for _ in 0..(20.0 * 48000.0 / 128.0) as usize {
            e.render(&mut buf);
            sum += buf.iter().map(|f| (f[0] as f64).abs()).sum::<f64>();
            melody_meter = melody_meter.max(e.status().roles[melody]);
        }
        (sum, melody_meter, e.status().bands)
    };
    let (open, open_meter, bands) = render(ctx);
    let (shut, shut_meter, _) = render(muted);
    assert!((open - shut).abs() > 1.0, "muting the melody changes the sound");
    assert!(open_meter > 0.1, "the melody's meter moves when it plays: {open_meter}");
    assert_eq!(shut_meter, 0.0, "and stays still when it is muted");
    assert!(bands.iter().filter(|&&b| b > 1e-4).count() >= 5, "the spectrum reads the music across its bands: {bands:?}");
}

/// THE PAGE'S ANALYSER IS WITHIN THE BUDGET (music.md section 14; music-page.md section 12): the
/// calm score with the Music page open (the spectrum's ten band-pass filters and the role meters)
/// still averages at most 0.05 ms a 128-frame block plus a fifth for the page, and the analyser
/// alone adds well under that. Best of five seeds, the slowest 2% of blocks set aside as the budget
/// test does (the OS taking the core mid-block).
///
/// Measured 2026-11-13 (release-optimised crate, the operator's i7-8700K with other agents
/// building): 0.031 to 0.039 ms a block off, the analyser about 0.007 ms (eight times the band work
/// added 0.055), under the noise of one run. Seen RED: with the band filters run eight times over,
/// "with the page open: 0.0864 ms a block (budget 0.05 + 0.01 for the page)".
#[test]
fn the_pages_analyser_is_within_the_budget() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let cost = |analyse: bool| {
        let mut best = f64::MAX;
        for seed in [1u64, 2, 3, 4, 5] {
            let ctx = MusicContext { mood, analyse, ..Default::default() };
            let mut s = ScoreEngine::new(res(&lib), 48000.0, seed, ctx);
            crate::render::render_score(&mut s, 48000.0, 2.0);
            let mut times = Vec::new();
            for _ in 0..(8.0 * 48000.0 / 128.0) as usize {
                let mut buf = [[0f32; 2]; 128];
                let t0 = std::time::Instant::now();
                s.render(&mut buf);
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(|a, b| a.total_cmp(b));
            let kept = &times[..(times.len() as f64 * 0.98) as usize];
            best = best.min(kept.iter().sum::<f64>() / kept.len() as f64);
        }
        best
    };
    let off = cost(false);
    let on = cost(true);
    println!("the calm score: {off:.4} ms a block, {on:.4} ms with the Music page's analyser ({:+.4})", on - off);
    assert!(on <= 0.06, "with the page open: {on:.4} ms a block (budget 0.05 + 0.01 for the page)");
    assert!(on - off <= 0.015, "the analyser alone: {:.4} ms a block (about 0.007 when measured)", on - off);
}
