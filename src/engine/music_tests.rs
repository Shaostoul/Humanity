// Tests for engine/music.rs's runtime: where the music is heard from, its gains and ducks, the build
// on its worker thread, a new score taking over at a phrase, and the reuse switch. The runtime is
// driven through an offline mixer: each playing score is a real `ScoreEngine` rendered here, so
// phrases pass in the tests as they do on kira's audio thread.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use humanity_music::{BedContext, BedEngine, BedStatus, MusicContext, MusicSettings, MusicStatus, ScoreEngine};

use super::*;

/// Kira's place in the tests: every score and bed started, rendered offline on demand.
#[derive(Default)]
struct Offline {
    scores: Vec<Arc<Mutex<ScoreEngine>>>,
}

struct OfflineScore(Arc<Mutex<ScoreEngine>>);

impl ScorePlay for OfflineScore {
    fn set_context(&mut self, ctx: MusicContext) {
        self.0.lock().unwrap().set_context(ctx);
    }
    fn status(&mut self) -> MusicStatus {
        self.0.lock().unwrap().status()
    }
}

impl Drop for OfflineScore {
    fn drop(&mut self) {
        self.0.lock().unwrap().stop();
    }
}

struct OfflineBed(BedContext);

impl BedPlay for OfflineBed {
    fn set_context(&mut self, ctx: BedContext) {
        self.0 = ctx;
    }
    fn status(&mut self) -> BedStatus {
        BedStatus::default()
    }
}

impl Mixer for Offline {
    fn play_score(&mut self, engine: ScoreEngine) -> Result<Box<dyn ScorePlay>, String> {
        let e = Arc::new(Mutex::new(engine));
        self.scores.push(e.clone());
        Ok(Box::new(OfflineScore(e)))
    }
    fn play_bed(&mut self, engine: BedEngine) -> Result<Box<dyn BedPlay>, String> {
        drop(engine);
        Ok(Box::new(OfflineBed(BedContext::default())))
    }
}

impl Offline {
    /// Render `seconds` of every score still sounding.
    fn render(&mut self, seconds: f32) {
        let mut buf = vec![[0.0f32; 2]; 1024];
        let blocks = (seconds * 48000.0 / 1024.0).ceil() as usize;
        for s in &self.scores {
            let mut e = s.lock().unwrap();
            if e.finished() {
                continue;
            }
            for _ in 0..blocks {
                e.render(&mut buf);
            }
        }
    }
}

fn inputs(data: &Path) -> Inputs {
    Inputs {
        settings: MusicSettings::default(),
        master: 1.0,
        music_volume: 0.7,
        hearing: Hearing::World,
        aboard: true,
        video_sound: false,
        // Noon in the Commons: the calm mood (data/music/places.ron).
        place: Here { zone: "commons".into(), hour: 12.0, ..Default::default() },
        intensity: 0.0,
        breach: false,
        data_dir: data.to_path_buf(),
        dt: 1.0 / 60.0,
        reuse: true,
        generation: 0,
        restart: false,
    }
}

/// A data folder of the music alone (no recordings: every part synthesised, so a build takes a few
/// milliseconds) with the calm mood made quick: two-bar phrases at 120 bpm, so a phrase passes in
/// four seconds of audio.
fn quick_music(tag: &str) -> humanity_core::test_temp::TempPath {
    let repo = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let t = humanity_core::test_temp::dir(tag);
    let copy = |from: &Path, to: &Path| {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_file() {
                let text = std::fs::read_to_string(&p).unwrap();
                // The recorded instruments stay behind (their samples are not copied).
                if !humanity_music::library::is_recorded_definition(&text) {
                    std::fs::write(to.join(e.file_name()), text).unwrap();
                }
            }
        }
    };
    for d in ["instruments", "moods", "styles"] {
        copy(&repo.join("music").join(d), &t.join("data/music").join(d));
    }
    for f in ["intensity.ron", "packs.ron", "parts.ron", "places.ron", "tones.ron"] {
        std::fs::copy(repo.join("music").join(f), t.join("data/music").join(f)).unwrap();
    }
    std::fs::create_dir_all(t.join("data/audio")).unwrap();
    std::fs::copy(repo.join("audio/hum_sources.ron"), t.join("data/audio/hum_sources.ron")).unwrap();
    edit_mood(&t.join("data"), |s| s.replace("phrase_bars: [8, 16]", "phrase_bars: [2]").replace("chord_bars: (2, 3)", "chord_bars: (1, 1)").replace("tempo_bpm: (56.0, 64.0)", "tempo_bpm: (120.0, 120.0)"));
    t
}

fn edit_mood(data: &Path, f: impl Fn(&str) -> String) {
    let p = data.join("music/moods/mothership_calm.ron");
    let s = std::fs::read_to_string(&p).unwrap();
    let edited = f(&s);
    assert_ne!(edited, s, "the edit changed the mood file");
    std::fs::write(&p, edited).unwrap();
}

/// Step the runtime `frames` times at 60 fps, rendering a frame's worth of audio each time; the
/// longest single step, milliseconds.
fn run(rt: &mut MusicRuntime, mix: &mut Offline, inp: &Inputs, frames: usize) -> f64 {
    let mut worst = 0.0f64;
    for _ in 0..frames {
        let t0 = Instant::now();
        rt.step(Some(mix as &mut dyn Mixer), inp);
        worst = worst.max(t0.elapsed().as_secs_f64() * 1000.0);
        mix.render(inp.dt);
    }
    worst
}

/// Step until `done`, waiting for each build the runtime starts (at most `max_frames` frames).
fn run_until(rt: &mut MusicRuntime, mix: &mut Offline, inp: &Inputs, max_frames: usize, done: impl Fn(&MusicReadout) -> bool) -> MusicReadout {
    for _ in 0..max_frames {
        let r = rt.step(Some(mix as &mut dyn Mixer), inp);
        if done(&r) {
            return r;
        }
        rt.wait_for_build();
        mix.render(inp.dt);
    }
    panic!("not reached in {max_frames} frames");
}

/// ESCAPE KEEPS THE WORLD'S MUSIC, 6 dB DOWN (BUG-269): out of the world onto a page, the page is
/// "over the world" (the world still loaded behind it) until the person moves on to another page or
/// goes back in; the title screen and a page reached from other pages are the app's pages. RED
/// first (2026-11-07) against the old rule (in the world only when no page is open: a page over the
/// world was any other page): "Escape onto the Chat page keeps the world (left: Page, right:
/// OverWorld)".
#[test]
fn escape_keeps_the_world_heard_until_the_person_moves_on() {
    let (h, kept) = hearing(true, None, true, GuiPage::Chat);
    assert_eq!(h, Hearing::OverWorld, "Escape onto the Chat page keeps the world");
    let (h, kept) = hearing(false, kept, true, GuiPage::Chat);
    assert_eq!(h, Hearing::OverWorld, "and it stays while that page is open");
    let (h, kept) = hearing(false, kept, true, GuiPage::Inventory);
    assert_eq!((h, kept), (Hearing::Page, None), "moving on to another page: the app's pages' rule");
    let (h, _) = hearing(false, kept, true, GuiPage::Chat);
    assert_eq!(h, Hearing::Page, "and coming back to the first page does not bring it back");
    assert_eq!(hearing(false, None, true, GuiPage::None).0, Hearing::World);
    assert_eq!(hearing(true, None, true, GuiPage::MainMenu).0, Hearing::Page, "the title screen is never over the world");
    assert_eq!(hearing(true, None, false, GuiPage::Chat).0, Hearing::Page, "no world loaded, no world to hear");
}

/// THE GAINS: in the world aboard, the music at Master x Music and undocked; a page over the world
/// keeps the music and the hum 6 dB down; a chat video's sound ducks the music by 12 dB and leaves
/// the hum; an app page plays no music unless Settings turns it on there; the hum follows Ambience
/// and its level, never the Music Volume (BUG-272). RED first (2026-11-07) against the old gains
/// (music only in the world, no duck): "a page over the world keeps the music (left: false, right:
/// true)".
#[test]
fn the_world_behind_a_page_is_6_db_down_and_a_video_ducks_the_music() {
    let data = PathBuf::from("data");
    let mut inp = inputs(&data);
    let world = gains(&inp);
    assert!(world.music && world.hum && world.duck_db == 0.0);
    assert!((world.music_gain - 0.7).abs() < 1e-6);
    inp.hearing = Hearing::OverWorld;
    let over = gains(&inp);
    assert_eq!(over.music, true, "a page over the world keeps the music");
    assert_eq!(over.duck_db, OVER_WORLD_DB);
    assert!(over.hum && (over.hum_gain / world.hum_gain - 0.501).abs() < 0.002, "and the hum, 6 dB down ({} of {})", over.hum_gain, world.hum_gain);
    inp.hearing = Hearing::World;
    inp.video_sound = true;
    let video = gains(&inp);
    assert_eq!(video.duck_db, VIDEO_DUCK_DB, "a chat video ducks the music");
    assert_eq!(video.hum_gain, world.hum_gain, "and leaves the hum");
    inp.hearing = Hearing::OverWorld;
    assert_eq!(gains(&inp).duck_db, OVER_WORLD_DB + VIDEO_DUCK_DB, "both at once add up");
    inp.video_sound = false;
    inp.hearing = Hearing::Page;
    assert!(!gains(&inp).music && !gains(&inp).hum, "an app page plays no music by default");
    inp.settings.music_on_pages = true;
    assert!(gains(&inp).music && gains(&inp).duck_db == 0.0, "unless Settings turns it on there");
    inp.hearing = Hearing::World;
    inp.music_volume = 0.0;
    assert_eq!(gains(&inp).hum_gain, world.hum_gain, "the music's volume does not move the hum");
}

/// THE BUILD RUNS OFF THE GAME THREAD, AND THE SCORE STARTS WHEN IT IS DONE (BUG-270): entering the
/// world with the real data (the core set's recordings decoded and levelled), no frame waits for the
/// build: the longest step while it runs is a few milliseconds, against the build's own time on its
/// thread (printed: the stall each world entry used to cost the game thread). RED first (2026-11-07)
/// with `start_build` running `build` inline, as the old frame did: "no frame waits for the build:
/// the longest frame took 3229.8 ms (the build 3229.5 ms)" (a debug build, its decoder unoptimised;
/// BUG-270 in docs/BUGS.md has the release measurement).
#[test]
fn the_build_runs_off_the_game_thread() {
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut rt = MusicRuntime::default();
    let mut mix = Offline::default();
    let inp = inputs(&data);
    let mut worst = 0.0f64;
    let t0 = Instant::now();
    let mut started = None;
    for _ in 0..20_000 {
        let s = Instant::now();
        let r = rt.step(Some(&mut mix as &mut dyn Mixer), &inp);
        worst = worst.max(s.elapsed().as_secs_f64() * 1000.0);
        if r.score.is_some() {
            started = Some(r);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let r = started.expect("the score starts once its build is done");
    let build_ms = r.build_ms.expect("the build's time is reported");
    println!("the build took {build_ms:.1} ms on its thread; the score started {:.0} ms after the first frame; the longest frame {worst:.2} ms", t0.elapsed().as_secs_f64() * 1000.0);
    assert!(worst < 25.0 && worst < build_ms as f64 / 4.0, "no frame waits for the build: the longest frame took {worst:.1} ms (the build {build_ms:.1} ms)");
    assert_eq!(r.pieces, 1);
}

/// WITH REUSE OFF, NOTHING STALE SURVIVES A PHRASE (BUG-267): the score playing, a mood file edited
/// on disk (its tempo) reaches the music: a new score built from the files read again takes over at
/// a phrase and plays the new tempo. With reuse on, the same edit does not reach the playing score
/// (the data is read when a score starts). RED first (2026-11-07) against the old runtime (the data
/// read once and kept while the score played, whatever the switch: the rebuild each phrase taken
/// out): "not reached in 2400 frames" (40 s of audio, ten phrases, still at 120 bpm).
#[test]
fn with_reuse_off_an_edited_mood_reaches_the_playing_score() {
    let t = quick_music("music_reuse_off");
    let data = t.join("data");
    let mut inp = inputs(&data);
    let mut rt = MusicRuntime::default();
    let mut mix = Offline::default();
    let r = run_until(&mut rt, &mut mix, &inp, 600, |r| r.score.is_some_and(|s| s.phrase >= 1));
    assert!((r.score.unwrap().tempo_bpm - 120.0).abs() < 0.5, "the quick mood plays at 120: {:?}", r.score);
    // Reuse on: the edit waits for the next score.
    edit_mood(&data, |s| s.replace("tempo_bpm: (120.0, 120.0)", "tempo_bpm: (90.0, 90.0)"));
    run(&mut rt, &mut mix, &inp, 600);
    let st = rt.step(Some(&mut mix as &mut dyn Mixer), &inp).score.unwrap();
    assert!((st.tempo_bpm - 120.0).abs() < 0.5, "with reuse on the playing score keeps what it was built from: {} bpm", st.tempo_bpm);
    // Reuse off: within two phrases (4 s each) a new score from the edited file takes over.
    inp.reuse = false;
    let r = run_until(&mut rt, &mut mix, &inp, 2400, |r| r.score.is_some_and(|s| (s.tempo_bpm - 90.0).abs() < 0.5));
    assert!(r.pieces >= 2, "a new score took over: {} pieces", r.pieces);
    let tempo = r.score.unwrap().tempo_bpm;
    assert!((tempo - 90.0).abs() < 0.5, "with reuse off the edited tempo reaches the playing score ({tempo:.1} bpm)");
}

/// AN INSTALL OR A REMOVAL REACHES THE PLAYING MUSIC AT THE NEXT PHRASE (BUG-266): the score
/// playing, the install generation moves (Settings > Audio > Instruments finished an install), and a
/// new score built from what is installed now takes over at a phrase, not after the music has been
/// let go for 30 s. RED first (2026-11-07) against the old runtime (a score kept until let go: the
/// rebuild on a new install generation taken out): "not reached in 1200 frames" (one piece only).
#[test]
fn an_install_reaches_the_playing_music_at_the_next_phrase() {
    let t = quick_music("music_install");
    let data = t.join("data");
    let mut inp = inputs(&data);
    let mut rt = MusicRuntime::default();
    let mut mix = Offline::default();
    run_until(&mut rt, &mut mix, &inp, 600, |r| r.score.is_some_and(|s| s.phrase >= 1));
    inp.generation = 1;
    let r = run_until(&mut rt, &mut mix, &inp, 1200, |r| r.pieces >= 2);
    assert!(r.pieces >= 2, "an install reaches the playing music: {} pieces after the install", r.pieces);
    // The old score fades out once the new one has taken over (the score's fade: a 1.5 s time
    // constant, so under -40 dB in about 7 s and ended after about 13).
    run(&mut rt, &mut mix, &inp, 900);
    assert!(mix.scores.first().is_some_and(|s| s.lock().unwrap().finished()), "the old score fades out and ends");
}

/// SETTINGS > MUSIC LISTS EVERY STYLE FROM THE DATA, MUSIC PLAYED OR NOT (BUG-273): with no audio
/// device and nothing playing, the music's data is read on the worker and the readout lists every
/// style file (lo-fi beside the default) and the tones. RED first (2026-11-07) against the old frame
/// (the data read only when a sound started): "every style is listed (left: [], right: [lofi,
/// mothership_calm])".
#[test]
fn every_style_is_listed_before_any_music_plays() {
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut inp = inputs(&data);
    inp.hearing = Hearing::Page;
    let mut rt = MusicRuntime::default();
    let mut r = rt.step(None, &inp);
    for _ in 0..1000 {
        if !r.styles.is_empty() {
            break;
        }
        rt.wait_for_build();
        r = rt.step(None, &inp);
    }
    let ids: Vec<String> = r.styles.iter().map(|(id, _, _)| id.clone()).collect();
    let mut files: Vec<String> = std::fs::read_dir(data.join("music/styles")).unwrap().flatten().map(|e| e.path().file_stem().unwrap().to_string_lossy().to_string()).collect();
    files.sort();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(sorted, files, "every style is listed");
    assert_eq!(ids.first().map(String::as_str), Some(humanity_music::library::DEFAULT_STYLE), "the default first");
    assert!(!r.tones.is_empty(), "and the tones");
    assert!(r.score.is_none() && !rt.building());
}

/// THE SCORE WITH THE CORE SET'S RECORDINGS, MEASURED (BUG-266; music.md section 14): each calm
/// mood in the default style, and lo-fi, built as the game builds it (the recordings of the parts
/// those styles sound, decoded and levelled) and rendered for 60 s offline in 128-frame blocks,
/// with the app's BS.1770 meter: integrated loudness, true peak, the cost per block (mean and 99th
/// percentile) and the decoded recordings' memory. Not a gate (the cost of a debug build's
/// surroundings varies); the renderer and the voices are the music crate's, optimised in every
/// build. Run by hand: node scripts/lib-tests.js the_score_with_recordings_measured --ignored --nocapture
#[test]
#[ignore = "a measurement printed for the record; run: node scripts/lib-tests.js the_score_with_recordings_measured --ignored --nocapture"]
fn the_score_with_recordings_measured() {
    use humanity_core::audio::loudness::LoudnessMeter;
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    for (mood, style) in [("mothership_calm", "mothership_calm"), ("night_watch", "mothership_calm"), ("garden_morning", "mothership_calm"), ("menu", "mothership_calm"), ("mothership_calm", "lofi"), ("action", "mothership_calm")] {
        let mut bank = SampleBank::new();
        let built = build(&data, 432.0, true, &mut bank, &[style.to_string()]).expect("builds");
        let lib = built.lib.clone();
        let recorded: Vec<String> = built.plays.iter().filter(|(_, p)| matches!(p, crate::instrument_packs::score::PartPlays::Recorded { .. })).map(|(p, _)| p.clone()).collect();
        let si = lib.style_index(style).unwrap() as u16;
        let intensity = if mood == "action" { 0.9 } else { 0.3 };
        let ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index(mood).unwrap() as u16, style_a: si, style_b: si, blend: 0.0, intensity, duck_db: 0.0 };
        let mut e = ScoreEngine::new(built.score.expect("a score"), 48000.0, 20261106, ctx);
        let mut meter = LoudnessMeter::new(48000, 2);
        let mut buf = [[0.0f32; 2]; 128];
        let mut times = Vec::new();
        let mut voices = 0u16;
        for _ in 0..(60.0 * 48000.0 / 128.0) as usize {
            let t0 = Instant::now();
            e.render(&mut buf);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            voices = voices.max(e.status().voices);
            for f in &buf {
                meter.push_frame(f);
            }
        }
        meter.finish();
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p99 = times[times.len() * 99 / 100];
        println!(
            "{mood}/{style}: {:.1} LUFS, true peak {:.2} dBTP, {mean:.4} ms mean and {p99:.4} ms at the 99th percentile per 128-frame block, {voices} voices at most, recordings {:.1} MiB decoded (built in {:.0} ms), recorded parts {recorded:?}",
            meter.integrated().unwrap_or(f64::NEG_INFINITY),
            meter.true_peak_dbtp(),
            bank.bytes() as f64 / 1048576.0,
            built.took_ms
        );
    }
}
