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
    /// Seconds of audio rendered so far (the tests' clock).
    now_s: f64,
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
        self.now_s += (blocks * 1024) as f64 / 48000.0;
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
        page_choice: None,
        favourites: Arc::default(),
        analyse: false,
    }
}

/// A data folder of the music alone (no recordings: every part synthesised, so a build takes a few
/// milliseconds) with the calm mood made quick: two-bar phrases at 120 bpm, so a phrase passes in
/// four seconds of audio.
fn quick_music(tag: &str) -> humanity_core::test_temp::TempPath {
    let t = music_alone(tag);
    edit_mood(&t.join("data"), |s| s.replace("phrase_bars: [8, 16]", "phrase_bars: [2]").replace("chord_bars: (2, 3)", "chord_bars: (1, 1)").replace("tempo_bpm: (56.0, 64.0)", "tempo_bpm: (120.0, 120.0)"));
    t
}

/// The music's data alone (no recordings), its moods as shipped.
fn music_alone(tag: &str) -> humanity_core::test_temp::TempPath {
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
    for d in ["instruments", "moods", "genres"] {
        copy(&repo.join("music").join(d), &t.join("data/music").join(d));
    }
    for f in ["intensity.ron", "packs.ron", "parts.ron", "pieces.ron", "places.ron", "tones.ron"] {
        std::fs::copy(repo.join("music").join(f), t.join("data/music").join(f)).unwrap();
    }
    std::fs::create_dir_all(t.join("data/audio")).unwrap();
    std::fs::copy(repo.join("audio/hum_sources.ron"), t.join("data/audio/hum_sources.ron")).unwrap();
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
    assert!(gains(&inp).music && !gains(&inp).hum && gains(&inp).duck_db == 0.0, "an app page plays the music by default, without the ship's hum");
    inp.settings.music_on_pages = false;
    assert!(!gains(&inp).music, "unless Settings turns it off there");
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
///
/// UP TO THREE TAKES (2026-11-14): after the time deadline below, the bound still failed four times
/// on 2026-11-13 (data/coordination/flaky-tests.jsonl): longest frames of 103, 118, 134 and 307 ms
/// with the build on its thread for 17 to 39 s while other worktrees compiled. Those are the OS
/// taking the core away from the test thread somewhere in a 20 to 40 s run, which no fixed bound
/// can rule out. So a take whose longest frame is over the bound is taken again from scratch (a
/// fresh runtime and build), up to three takes, and the test passes on the first take that holds.
/// That repeats the MEASUREMENT, not the assertion: the build run inline stalls the frame that runs
/// it for the build's whole time in EVERY take, so it can never find a passing take. Red evidence,
/// 2026-11-14, with `start_build` waiting for its build thread before returning (the build inline
/// in effect): takes of 3103, 3172 and 3233 ms longest frames, "no frame waits for the build: in
/// none of three takes; the last: the longest frame took 3232.7 ms (the build 3231.3 ms)".
#[test]
fn the_build_runs_off_the_game_thread() {
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let inp = inputs(&data);
    let mut last = String::new();
    for take in 1..=3 {
        let mut rt = MusicRuntime::default();
        let mut mix = Offline::default();
        let mut worst = 0.0f64;
        let t0 = Instant::now();
        let mut started = None;
        // A deadline in time, not a count of frames: under a loaded machine (other worktrees
        // compiling) the debug build took 21.5 s on 2026-11-13, and later did not finish within
        // 20,000 frames of about 2 ms (flaky three times that day, data/coordination/flaky-tests.jsonl).
        while t0.elapsed() < std::time::Duration::from_secs(300) {
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
        println!("take {take}: the build took {build_ms:.1} ms on its thread; the score started {:.0} ms after the first frame; the longest frame {worst:.2} ms", t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(r.pieces, 1);
        // 100 ms, not 25: a loaded machine stretched one step to 28.06 ms (2026-11-13) with the build
        // on its thread, while the build run inline stalls a frame for its whole time (3229.8 ms in
        // the red run), so the bound still tells the two apart thirty times over.
        if worst < 100.0 && worst < build_ms as f64 / 4.0 {
            return;
        }
        last = format!("the longest frame took {worst:.1} ms (the build {build_ms:.1} ms)");
    }
    panic!("no frame waits for the build: in none of three takes; the last: {last}");
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

/// How long a genre change takes to reach the output may be, seconds of audio: the playing score's
/// next bar line (4.3 s at the calm mood's slowest, 56 bpm), the new score's fade to half its level
/// (a 1.5 s time constant: 1.04 s), and room for the build (the core set's electro swing recordings
/// decode in tens of milliseconds in an optimised build, BUG-297).
const GENRE_CHANGE_HEARD_S: f64 = 6.0;

/// Play the calm mood `into_s` seconds, choose electro swing, and step until a score PLAYS electro
/// swing (its phrase at the genre's 128 bpm; the calm mood is 56 to 64) at half its level or more;
/// the seconds of audio from the choice to that. (What the score is told is not evidence: the
/// playing score hears of the choice at once, and a first version of this check, reading the
/// context, passed against the slow runtime at 0.0 s.)
fn genre_change_heard_after(data: &Path, into_s: f32, reuse: bool, seed: u64) -> f64 {
    let mut inp = inputs(data);
    inp.reuse = reuse;
    let mut rt = MusicRuntime { seed, ..Default::default() };
    let mut mix = Offline::default();
    run_until(&mut rt, &mut mix, &inp, 600, |r| r.score.is_some());
    run(&mut rt, &mut mix, &inp, (into_s * 60.0) as usize);
    inp.settings.genre = "electro_swing".into();
    let t0 = mix.now_s;
    for _ in 0..(150 * 60) {
        rt.step(Some(&mut mix as &mut dyn Mixer), &inp);
        let heard = mix.scores.iter().any(|s| {
            let e = s.lock().unwrap();
            let st = e.status();
            !e.finished() && (st.tempo_bpm - 128.0).abs() < 0.5 && st.gain >= 0.5 * e.context().gain
        });
        if heard {
            return mix.now_s - t0;
        }
        rt.wait_for_build();
        mix.render(inp.dt);
    }
    f64::INFINITY
}

/// A GENRE CHANGE IS HEARD WITHIN SECONDS (BUG-297, the operator 2026-11-13: "I clicked to enable
/// electro swing but, the transition was really slow, felt like it took a minute or so."). From the
/// calm mood as it ships (8 to 16 bar phrases at 56 to 64 bpm), electro swing is chosen at several
/// moments into the piece, with the reuse switch on and off, and a score of electro swing must play
/// at half its level within [`GENRE_CHANGE_HEARD_S`] of the choice: the new score takes over at the
/// playing one's next bar line, crossfading, and a build begun for the old genre is not played.
/// RED first (2026-11-13) against the runtime of v0.1558 (the playing score switched genre only at
/// its next phrase, with stand-ins, and the rebuilt score took over a phrase after that, never before
/// the playing score's second): "a genre change is heard within 6 s: the worst 62.4 s ([(3.0, t,
/// 58.4), (3.0, f, 62.4), (11.0, t, 52.3), (11.0, f, 48.5), (23.0, t, 2.2), (23.0, f, 35.0), (41.0,
/// t, 40.6), (41.0, f, 13.8)])" (a session seed then; the seeds are fixed now).
#[test]
fn a_genre_change_is_heard_within_seconds() {
    let t = music_alone("music_genre_change");
    let data = t.join("data");
    let mut times = Vec::new();
    for (k, into_s) in [3.0f32, 11.0, 23.0, 41.0].into_iter().enumerate() {
        for reuse in [true, false] {
            times.push((into_s, reuse, genre_change_heard_after(&data, into_s, reuse, 20261113 + k as u64)));
        }
    }
    let worst = times.iter().map(|t| t.2).fold(0.0f64, f64::max);
    println!("a genre change heard after (seconds into the piece, reuse, seconds): {times:.1?}");
    assert!(worst <= GENRE_CHANGE_HEARD_S, "a genre change is heard within {GENRE_CHANGE_HEARD_S} s: the worst {worst:.1} s ({times:.1?})");
}

/// A NEW PIECE EVERY FEW MINUTES, AT A PHRASE; THE SAME PIECE CARRIED ON BY A REBUILD (2026-11-13,
/// docs/design/music.md section 7c): with "A new piece" at a quarter of a minute, the quick calm mood
/// (a phrase every 4 s) begins a new piece, a new seed, after its 15 s, each new score starting as
/// the old one begins a phrase; with Never one piece plays the whole minute; and a rebuild that is
/// not a new piece (a pack installed) keeps the piece's seed and carries on its form from the phrase
/// it was at. RED first (2026-11-13) with the piece timer taken out: "a new piece after its minutes:
/// 2 seeds in 60 s ([0, 20261113])" (the 0 a status read before the first block; skipped since).
#[test]
fn a_new_piece_begins_after_its_minutes_and_a_rebuild_carries_the_piece_on() {
    let t = quick_music("music_pieces");
    let data = t.join("data");
    let seeds_over = |minutes: f32, seconds: f32| -> (Vec<u64>, Vec<bool>) {
        let mut inp = inputs(&data);
        inp.settings.piece_minutes = minutes;
        let mut rt = MusicRuntime { seed: 20261113, ..Default::default() };
        let mut mix = Offline::default();
        let mut seeds: Vec<u64> = Vec::new();
        let mut at_phrase: Vec<bool> = Vec::new();
        for _ in 0..(seconds * 60.0) as usize {
            let r = rt.step(Some(&mut mix as &mut dyn Mixer), &inp);
            if let Some(s) = r.score {
                if s.piece_seed != 0 && seeds.last() != Some(&s.piece_seed) {
                    seeds.push(s.piece_seed);
                    // The old score had just begun a phrase when the new one started: it is in the
                    // first bar of its phrase (a frame is 17 ms; a bar here is 2 s).
                    let old_bar = mix.scores.len().checked_sub(2).map(|i| mix.scores[i].lock().unwrap().status().bar);
                    at_phrase.push(old_bar.is_none_or(|b| b == 0));
                }
            }
            rt.wait_for_build();
            mix.render(inp.dt);
        }
        (seeds, at_phrase)
    };
    let (seeds, at_phrase) = seeds_over(0.25, 60.0);
    assert!(seeds.len() >= 3, "a new piece after its minutes: {} seeds in 60 s ({seeds:?})", seeds.len());
    let mut uniq = seeds.clone();
    uniq.dedup();
    assert_eq!(uniq.len(), seeds.len(), "each piece a new seed: {seeds:?}");
    assert!(at_phrase.iter().all(|&x| x), "each new piece at a phrase: {at_phrase:?}");
    assert_eq!(seeds[0], 20261113, "the first piece's seed is the session's (HUMANITY_MUSIC_SEED for a rig)");
    let (never, _) = seeds_over(0.0, 60.0);
    assert_eq!(never.len(), 1, "Never: one piece the whole minute ({never:?})");
    // A pack installed mid-piece: a new score, the same piece, carried on from where it was.
    let mut inp = inputs(&data);
    inp.settings.piece_minutes = 0.0;
    let mut rt = MusicRuntime { seed: 77, ..Default::default() };
    let mut mix = Offline::default();
    let before = run_until(&mut rt, &mut mix, &inp, 3000, |r| r.score.is_some_and(|s| s.form_pos >= 2)).score.unwrap();
    inp.generation = 1;
    // (A new score reports its status once it has rendered a block.)
    let after = run_until(&mut rt, &mut mix, &inp, 3000, |r| r.pieces >= 2 && r.score.is_some_and(|s| s.blocks > 0 || s.seconds > 0.0)).score.unwrap();
    assert_eq!(after.piece_seed, before.piece_seed, "a rebuild keeps the piece's seed");
    assert!(after.form_pos > before.form_pos && after.form_pos <= before.form_pos + 2, "and carries its form on: phrase {} of the piece, then {}", before.form_pos, after.form_pos);
}

/// A FAVOURITE ASSIGNED TO DANGER PLAYS WHEN THE MOOD TURNS TO DANGER (docs/design/music-page.md
/// section 7; the operator, 2026-11-13: "maybe seed X has a particular sound I love and want to
/// [for] combat"): calm, the session's own piece plays; the intensity rising into the tension band,
/// the favourite kept as tension's "my music" takes over (its seed, its genre) within a few seconds;
/// falling back to calm, new pieces again. And the Music page's choice, while it is open, plays in
/// place of both.
///
/// Seen RED (2026-11-13), before the runtime knew favourites: "not reached in 3000 frames" at the
/// danger step (the session's piece played on through danger).
#[test]
fn a_favourite_assigned_to_danger_plays_when_the_mood_turns_to_danger() {
    use humanity_music::favourite::{Favourite, Favourites};
    let t = quick_music("music_my_danger");
    let data = t.join("data");
    let mut inp = inputs(&data);
    let mut favs = Favourites::default();
    let id = favs.add(Favourite { id: 0, name: "Fight".into(), genre: "lofi".into(), mood: "tension".into(), seed: 777, tune: Default::default() });
    favs.assign("tension", Some(id));
    inp.favourites = Arc::new(favs);
    let mut rt = MusicRuntime { seed: 20261113, ..Default::default() };
    let mut mix = Offline::default();
    let calm = run_until(&mut rt, &mut mix, &inp, 3000, |r| r.score.is_some_and(|s| s.seconds > 0.5)).score.unwrap();
    assert_eq!(calm.piece_seed, 20261113, "calm: the session's own piece");
    inp.intensity = 0.5;
    let before = mix.now_s;
    run_until(&mut rt, &mut mix, &inp, 3000, |r| r.score.is_some_and(|s| s.piece_seed == 777 && s.seconds > 0.0));
    assert!(mix.now_s - before < 6.0, "the favourite takes over within a few seconds: {:.1} s", mix.now_s - before);
    assert_eq!(rt.score_genres, vec!["lofi".to_string()], "in the favourite's genre");
    // The danger mood comes at a bar line once the score has begun its first phrase.
    run_until(&mut rt, &mut mix, &inp, 3000, |r| r.mood == "tension" && r.score.is_some_and(|s| s.piece_seed == 777));
    inp.intensity = 0.0;
    run_until(&mut rt, &mut mix, &inp, 3000, |r| r.score.is_some_and(|s| s.piece_seed != 777 && s.seconds > 0.0));
    assert_eq!(rt.score_genres, vec![MusicSettings::default().genre], "calm again: the person's genre, a new piece");
    // The Music page open with Play here: its choice plays, danger or not.
    inp.intensity = 0.5;
    let tune = humanity_music::tune::Tune { transpose: 2, ..Default::default() };
    inp.page_choice = Some(MusicChoice { genre: "electro_swing".into(), mood: Some("mothership_calm".into()), intensity: Some(0.3), seed: 31, tune });
    let r = run_until(&mut rt, &mut mix, &inp, 3000, |r| r.score.is_some_and(|s| s.piece_seed == 31 && s.seconds > 0.0));
    assert_eq!(rt.score_genres, vec!["electro_swing".to_string()]);
    assert_eq!(r.mood, "mothership_calm", "the page's mood, not the game's danger");
    assert!(r.playing.as_ref().is_some_and(|p| p.choice.as_ref() == inp.page_choice.as_ref()), "the readout says what plays, for the page's piano roll");
}

/// SETTINGS > MUSIC LISTS EVERY GENRE FROM THE DATA, MUSIC PLAYED OR NOT (BUG-273): with no audio
/// device and nothing playing, the music's data is read on the worker and the readout lists every
/// genre file (lo-fi beside the default) and the tones. RED first (2026-11-07) against the old frame
/// (the data read only when a sound started): "every genre is listed (left: [], right: [lofi,
/// mothership_calm])".
#[test]
fn every_genre_is_listed_before_any_music_plays() {
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut inp = inputs(&data);
    inp.hearing = Hearing::Page;
    let mut rt = MusicRuntime::default();
    let mut r = rt.step(None, &inp);
    for _ in 0..1000 {
        if !r.genres.is_empty() {
            break;
        }
        rt.wait_for_build();
        r = rt.step(None, &inp);
    }
    let ids: Vec<String> = r.genres.iter().map(|g| g.id.clone()).collect();
    let mut files: Vec<String> = std::fs::read_dir(data.join("music/genres")).unwrap().flatten().map(|e| e.path().file_stem().unwrap().to_string_lossy().to_string()).collect();
    files.sort();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(sorted, files, "every genre is listed");
    assert_eq!(ids.first().map(String::as_str), Some(humanity_music::library::DEFAULT_GENRE), "the default first");
    assert!(!r.tones.is_empty(), "and the tones");
    assert!(r.score.is_none() && !rt.building());
}

/// THE SCORE WITH THE CORE SET'S RECORDINGS, MEASURED (BUG-266; music.md section 14): each calm
/// mood in the default genre, and lo-fi, built as the game builds it (the recordings of the parts
/// those genres sound, decoded and levelled) and rendered for 60 s offline in 128-frame blocks,
/// with the app's BS.1770 meter: integrated loudness, true peak, the cost per block (mean and 99th
/// percentile) and the decoded recordings' memory. Not a gate (the cost of a debug build's
/// surroundings varies); the renderer and the voices are the music crate's, optimised in every
/// build. Run by hand: node scripts/lib-tests.js the_score_with_recordings_measured --ignored --nocapture
/// With HUMANITY_MUSIC_WAV_DIR set, each render is also written there as a WAV
/// (`<mood>-<genre>.wav`), so a person can listen to the score as the app plays it, recordings and
/// all (music-render, in the music crate, plays the synthesised voices alone).
#[test]
#[ignore = "a measurement printed for the record; run: node scripts/lib-tests.js the_score_with_recordings_measured --ignored --nocapture"]
fn the_score_with_recordings_measured() {
    use humanity_core::audio::loudness::LoudnessMeter;
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    for (mood, genre) in [("mothership_calm", "mothership_calm"), ("night_watch", "mothership_calm"), ("garden_morning", "mothership_calm"), ("menu", "mothership_calm"), ("mothership_calm", "lofi"), ("mothership_calm", "electro_swing"), ("action", "mothership_calm")] {
        let mut bank = SampleBank::new();
        let built = build(&data, 432.0, true, &mut bank, &[genre.to_string()]).expect("builds");
        let lib = built.lib.clone();
        let recorded: Vec<String> = built.plays.iter().filter(|(_, p)| matches!(p, crate::instrument_packs::score::PartPlays::Recorded { .. })).map(|(p, _)| p.clone()).collect();
        let si = lib.genre_index(genre).unwrap() as u16;
        let intensity = if mood == "action" { 0.9 } else { 0.3 };
        let ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index(mood).unwrap() as u16, genre_a: si, genre_b: si, blend: 0.0, intensity, duck_db: 0.0, ..Default::default() };
        let mut e = ScoreEngine::new(built.score.expect("a score"), 48000.0, 20261106, ctx);

        // Offline, faster than real time: a streamed recording waits for its stream.

        e.set_offline(true);
        let mut meter = LoudnessMeter::new(48000, 2);
        let mut buf = [[0.0f32; 2]; 128];
        let mut times = Vec::new();
        let mut voices = 0u16;
        let wav_dir = std::env::var_os("HUMANITY_MUSIC_WAV_DIR").map(PathBuf::from);
        let mut frames: Vec<[f32; 2]> = Vec::new();
        for _ in 0..(60.0 * 48000.0 / 128.0) as usize {
            let t0 = Instant::now();
            e.render(&mut buf);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            voices = voices.max(e.status().voices);
            for f in &buf {
                meter.push_frame(f);
            }
            if wav_dir.is_some() {
                frames.extend_from_slice(&buf);
            }
        }
        meter.finish();
        if let Some(dir) = &wav_dir {
            let path = dir.join(format!("{mood}-{genre}.wav"));
            humanity_music::render::write_wav(&path, &frames, 48000).expect("the WAV is written");
            println!("  written: {}", path.display());
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p99 = times[times.len() * 99 / 100];
        println!(
            "{mood}/{genre}: {:.1} LUFS, true peak {:.2} dBTP, {mean:.4} ms mean and {p99:.4} ms at the 99th percentile per 128-frame block, {voices} voices at most, recordings {:.1} MiB decoded (built in {:.0} ms), recorded parts {recorded:?}",
            meter.integrated().unwrap_or(f64::NEG_INFINITY),
            meter.true_peak_dbtp(),
            bank.bytes() as f64 / 1048576.0,
            built.took_ms
        );
    }
}

/// ELECTRO SWING PLAYS RECORDINGS OUT OF THE BOX (2026-11-12): built as the game builds it, with
/// nothing installed beyond the core set that ships inside the app, the electro swing genre's
/// trumpet, trombone, clarinet, plucked double bass and guitar each play a RECORDING (the core set's
/// small trumpet, old trombone, clarinet, pizzicato bass and Gretsch, all CC0 from libraries already
/// approved and downloaded), not a synthesised stand-in, and Settings' stand-ins line for it is
/// empty; 60 s of it rendered offline (the music crate's loudness test's seed and length: a phrase
/// lasts 15 to 30 s at 128 bpm and each plays at its own dynamic level, so 20 s of one seed read 1.6 LU
/// away from 60 s of another) sit at the music target within 1 LU under the -1 dBTP ceiling, as its
/// synthesised stand-ins do in the crate's test. RED first (2026-11-12) with the core set's
/// trumpet definition moved aside: "electro swing's trumpet plays Synth { instrument: \"swing_horns\" }".
#[test]
fn electro_swing_plays_recordings_out_of_the_box() {
    use crate::instrument_packs::score::PartPlays;
    use humanity_core::audio::loudness::LoudnessMeter;
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut bank = SampleBank::new();
    let built = build(&data, 432.0, true, &mut bank, &["electro_swing".to_string()]).expect("builds");
    // (Since 2026-11-13 the comping guitar is the Martin steel-string, part `acoustic_guitar`, and a
    // piece's lead voices are among the violin, the tenor saxophone, the vibraphone and the piano.)
    for part in ["trumpet", "trombone", "clarinet", "double_bass_pizzicato", "acoustic_guitar", "solo_violin", "tenor_saxophone", "vibraphone", "piano"] {
        let plays = built.plays.iter().find(|(p, _)| p == part).map(|(_, p)| p.clone());
        assert!(matches!(plays, Some(PartPlays::Recorded { .. })), "electro swing's {part} plays {}", plays.map(|p| format!("{p:?}")).unwrap_or_else(|| "nothing".into()));
    }
    let lib = built.lib.clone();
    let gi = lib.genre_index("electro_swing").unwrap();
    // Its stand-ins (since the 2026-11-13 genre pass, the parts an optional pack brings: the
    // brushes, the honky-tonk piano, the tuba) each name the pack, and none is a part the core set
    // holds.
    let catalog = crate::instrument_packs::catalog::PackCatalog::load(&data).expect("packs.ron");
    let parts = crate::instrument_packs::catalog::Parts::load(&data).expect("parts.ron");
    let core_words: Vec<&str> = catalog.core.instruments.iter().filter_map(|i| parts.part(&i.part)).map(|p| p.words.as_str()).collect();
    for s in &built.stand_ins[gi] {
        assert!(s.pack.is_some(), "electro swing's stand-in {} names the pack that brings it", s.part);
        assert!(!core_words.contains(&s.part.as_str()), "electro swing's {} is in the core set, so plays its recording", s.part);
    }
    let ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index("mothership_calm").unwrap() as u16, genre_a: gi as u16, genre_b: gi as u16, blend: 0.0, intensity: 0.3, duck_db: 0.0, ..Default::default() };
    let mut e = ScoreEngine::new(built.score.expect("a score"), 48000.0, 20261106, ctx);

    // Offline, faster than real time: a streamed recording waits for its stream.

    e.set_offline(true);
    let mut meter = LoudnessMeter::new(48000, 2);
    let mut buf = [[0.0f32; 2]; 128];
    for _ in 0..(60.0 * 48000.0 / 128.0) as usize {
        e.render(&mut buf);
        for f in &buf {
            meter.push_frame(f);
        }
    }
    meter.finish();
    let lufs = meter.integrated().unwrap_or(f64::NEG_INFINITY);
    println!("electro swing with the core set's recordings: {lufs:.1} LUFS, true peak {:.2} dBTP", meter.true_peak_dbtp());
    assert!((lufs + 18.0).abs() <= 1.0, "electro swing with its recordings: {lufs:.1} LUFS, the target -18");
    assert!(meter.true_peak_dbtp() <= -1.0, "true peak {:.2} dBTP", meter.true_peak_dbtp());
}

/// THE MUSIC STARTS WHEN HUMANITYOS STARTS AND STAYS ON THE PAGES (2026-11-11, the operator: "we
/// should have the music start playing when HumanityOS starts", then, asked whether it should stop
/// at the first trip into the world: "Play on the app pages as well."): one switch, Settings >
/// Music > "Music on the app's pages", on by default, plays the score on the title screen and the
/// pages from the app's start and all session, a trip into the world and back included. It follows
/// Music on and the volumes, the switch turns it off, and an instance with no sound device (a
/// script's background launch, engine/launch_focus.rs) plays nothing.
///
/// Seen red 2026-11-11 against the two-switch code (the music at start ending at the first world
/// entry: the frame's `opening` input set false after it, and "Music on the app's pages" off by
/// default): "after a trip into the world and back, a page still plays the music" (left: false,
/// right: true).
#[test]
fn the_music_starts_with_the_app_and_a_script_launch_stays_silent() {
    let t = quick_music("music_at_start");
    let data = t.join("data");
    let mut inp = inputs(&data);
    inp.hearing = Hearing::Page;
    inp.aboard = false;
    assert_eq!(gains(&inp).music, true, "the title screen plays the music at the app's start");
    assert!((gains(&inp).music_gain - 0.7).abs() < 1e-6, "at the Music Volume times the Master");
    inp.master = 0.0;
    assert_eq!(gains(&inp).music_gain, 0.0, "the Master Volume at zero silences it");
    inp.master = 1.0;
    inp.settings.music_on = false;
    assert_eq!(gains(&inp).music, false, "Music off silences it");
    inp.settings.music_on = true;
    inp.settings.music_on_pages = false;
    assert_eq!(gains(&inp).music, false, "the switch turns the music on the pages off");
    inp.settings.music_on_pages = true;
    // A score really starts on a page at the app's start (the quick data builds in milliseconds),
    // plays on in the world, and is still playing on a page after the trip there and back.
    let mut rt = MusicRuntime::default();
    let mut mix = Offline::default();
    let r = run_until(&mut rt, &mut mix, &inp, 600, |r| r.score.is_some());
    assert!(r.device && r.score.is_some(), "a score plays at the app's start");
    inp.hearing = Hearing::World;
    inp.aboard = true;
    let r = run_until(&mut rt, &mut mix, &inp, 600, |r| r.score.is_some());
    assert!(r.score.is_some(), "the music plays in the world");
    inp.hearing = Hearing::Page;
    inp.aboard = false;
    inp.settings = MusicSettings::default();
    assert_eq!(gains(&inp).music, true, "after a trip into the world and back, a page still plays the music");
    run(&mut rt, &mut mix, &inp, 600);
    let r = rt.step(Some(&mut mix as &mut dyn Mixer), &inp);
    assert!(r.score.is_some(), "after a trip into the world and back, the score is still playing on the page");
    // No sound device, as a script's background launch has none: nothing plays, frame after frame.
    let mut silent = MusicRuntime::default();
    for _ in 0..300 {
        let r = silent.step(None, &inp);
        assert!(!r.device && r.score.is_none(), "an instance with no sound device plays nothing");
        silent.wait_for_build();
    }
}

/// PIECES TO LISTEN TO (2026-11-13, docs/design/music.md sections 7c and 7e): 60 s of each piece
/// HUMANITY_MUSIC_PIECES names (by default two electro swing seeds and one of the ship's own genre;
/// an item `genre:seed`, with `:kit=<id>` for the first seed from there that draws that kit and
/// `:<role>=<part>` for a lead voice, as the Music page chooses one), built as the game builds them
/// (the installed recordings) and written as WAVs into HUMANITY_MUSIC_WAV_DIR
/// (`<HUMANITY_MUSIC_WAV_PREFIX>-<genre>-seed-<seed>.wav`), each piece's key, form, tempo and lead
/// voice printed. An item `instrument:<id>` (or `instrument:<id>:<seconds>`) is one instrument by
/// itself instead (rungs E4 and E5: a drum in a groove, anything else chords held then arpeggiated).
/// Not a gate. Run by hand, for example:
/// HUMANITY_MUSIC_WAV_DIR=<dir> HUMANITY_MUSIC_PIECES=deep_dubstep:1,electro_swing:1:kit=brushes:bass=tuba
/// node scripts/lib-tests.js pieces_to_listen_to --ignored --nocapture
#[test]
#[ignore = "writes WAVs for a person to listen to; run: node scripts/lib-tests.js pieces_to_listen_to --ignored --nocapture"]
fn pieces_to_listen_to() {
    let Some(dir) = std::env::var_os("HUMANITY_MUSIC_WAV_DIR").map(PathBuf::from) else {
        println!("HUMANITY_MUSIC_WAV_DIR is not set: nothing written");
        return;
    };
    let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    // Which pieces (HUMANITY_MUSIC_PIECES, "genre:seed" items separated by commas; an item may add
    // ":kit=<id>", the first seed from there whose piece draws that kit, and ":<role>=<part>", a lead
    // voice chosen as the Music page would), and the files' prefix (HUMANITY_MUSIC_WAV_PREFIX).
    let pieces = std::env::var("HUMANITY_MUSIC_PIECES").unwrap_or_else(|_| "electro_swing:20261113,electro_swing:4242,mothership_calm:20261113".to_string());
    let prefix = std::env::var("HUMANITY_MUSIC_WAV_PREFIX").unwrap_or_else(|_| "piece".to_string());
    for item in pieces.split(',').filter(|s| !s.trim().is_empty()) {
        // "instrument:<id>[:<seconds>]": one instrument by itself (docs/design/music.md section 7f,
        // `render::render_demo`): a drum in a groove, anything else four chords held then arpeggiated.
        if let Some(rest) = item.trim().strip_prefix("instrument:") {
            let mut bits = rest.split(':');
            let id = bits.next().unwrap_or_default();
            let seconds: f64 = bits.next().and_then(|s| s.parse().ok()).unwrap_or(32.0);
            let lib = humanity_music::MusicLibrary::load(&data).expect("the music data");
            let r = humanity_music::render::render_demo(&lib, id, 432.0, seconds, 48000.0).unwrap_or_else(|e| panic!("{e}"));
            let path = dir.join(format!("{prefix}-instrument-{}.wav", id.replace('_', "-")));
            humanity_music::render::write_wav(&path, &r.frames, 48000).expect("the WAV is written");
            println!("{}: {id} by itself, {seconds:.0} s", path.display());
            continue;
        }
        let mut bits = item.trim().split(':');
        let genre = bits.next().unwrap_or("mothership_calm");
        let mut seed: u64 = bits.next().and_then(|s| s.parse().ok()).unwrap_or(20261113);
        let opts: Vec<(&str, &str)> = bits.filter_map(|b| b.split_once('=')).collect();
        let mut bank = SampleBank::new();
        let built = build(&data, 432.0, true, &mut bank, &[genre.to_string()]).expect("builds");
        let lib = built.lib.clone();
        let gi = lib.genre_index(genre).unwrap() as u16;
        let mut ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index("mothership_calm").unwrap() as u16, genre_a: gi, genre_b: gi, blend: 0.0, intensity: 0.3, duck_db: 0.0, ..Default::default() };
        let g = &lib.genres[gi as usize];
        let mut label = String::new();
        for (k, v) in &opts {
            if *k == "kit" {
                let want = g.kits.iter().position(|x| x.id == *v).expect("the genre has that kit") as u8;
                while humanity_music::piece::Piece::new(seed).kit(&g.kits.iter().map(|x| x.weight).collect::<Vec<_>>()) != Some(want) {
                    seed += 1;
                }
            } else if let Some(role) = humanity_music::tune::ROLE_IDS.iter().position(|r| r == k) {
                // A lead by its part, or (a lead with no part, deep dubstep's wavetable growl) its instrument.
                let i = g.leads.iter().position(|e| e.role.index() == role && (e.part.as_deref() == Some(*v) || e.instrument == *v)).expect("the genre offers that lead");
                ctx.tune.lead[role] = i as u16;
            }
            label.push_str(&format!("-{v}"));
        }
        let mut e = ScoreEngine::new(built.score.expect("a score"), 48000.0, seed, ctx);

        // Offline, faster than real time: a streamed recording waits for its stream.

        e.set_offline(true);
        let mut frames: Vec<[f32; 2]> = Vec::new();
        let mut buf = [[0.0f32; 2]; 128];
        for _ in 0..(60.0 * 48000.0 / 128.0) as usize {
            e.render(&mut buf);
            frames.extend_from_slice(&buf);
        }
        let path = dir.join(format!("{prefix}-{}{label}-seed-{seed}.wav", genre.replace('_', "-")));
        humanity_music::render::write_wav(&path, &frames, 48000).expect("the WAV is written");
        let c = e.composer();
        let melody = c.role_instruments()[humanity_music::mood::RoleKind::Melody.index()].and_then(|i| e.instruments().get(i as usize)).map(|i| i.id.clone()).unwrap_or_default();
        let form: String = c.piece_form().iter().map(|&l| (b'A' + l) as char).collect();
        println!("{}: {genre}, seed {seed}: key {} {:?}, form {form}, {:.0} bpm, melody {melody}", path.display(), names[c.key_pc().rem_euclid(12) as usize], c.mode(), c.tempo());
    }
}

/// A 60 s RENDER WITH THE FULL KEYS PACK, BEFORE AND AFTER (run by hand:
/// `HUMANITY_INSTRUMENT_PACKS_OUT=<folder> HUMANITY_MUSIC_WAV_DIR=<folder> node scripts/lib-tests.js
/// a_full_keys_render_is_the_same_streamed --ignored --nocapture`). Lo-fi's calm mood with the keys
/// pack (version 2) installed, its piano the full Steinway, rendered for 60 s twice: decoded whole as
/// before (offline, as fast as it goes), and streamed AS THE AUDIO THREAD PLAYS IT, paced at real
/// time with the worker thread running and never waited for (128-frame blocks kept at most 20 ms
/// ahead of the clock, a sound card's buffer). The two are compared sample by sample and both written
/// as WAVs (`st-full-keys-whole.wav`, `st-full-keys-streamed.wav`).
#[test]
#[ignore = "needs the staged keys pack; 60 s of real time; run by hand with HUMANITY_INSTRUMENT_PACKS_OUT"]
fn a_full_keys_render_is_the_same_streamed() {
    use crate::instrument_packs::score::PartPlays;
    use crate::instrument_packs::stream_tests::{install_staged, music_data};
    let data = music_data("st-keys-render");
    install_staged(&data, "keys").expect("the keys pack installs");
    let wav_dir = std::env::var_os("HUMANITY_MUSIC_WAV_DIR").map(PathBuf::from);
    let mut renders: Vec<Vec<[f32; 2]>> = Vec::new();
    for streamed in [false, true] {
        let mut bank = SampleBank::new();
        bank.reuse_override = Some(false);
        let built = {
            // The engine's own build, with streaming as asked.
            let (mut settings, _) = crate::instrument_packs::samples::stream_settings(&data);
            settings.enabled = streamed;
            let text = format!("StreamSettings(enabled: {}, stream_longer_than_s: {:?}, attack_s: {:?}, streams: {}, ring_s: {:?}, fade_ms: {:?}, poll_ms: {:?})", settings.enabled, settings.stream_longer_than_s, settings.attack_s, settings.streams, settings.ring_s, settings.fade_ms, settings.poll_ms);
            std::fs::write(data.join("music").join("streaming.ron"), text).unwrap();
            crate::engine::music::build(&data, 432.0, true, &mut bank, &["lofi".to_string()]).expect("builds")
        };
        assert_eq!(built.plays.iter().find(|(p, _)| p == "piano").map(|(_, p)| p.clone()), Some(PartPlays::Recorded { id: "grand_piano_steinway".into() }));
        let lib = built.lib.clone();
        let res = built.score.expect("a score");
        let recordings: usize = res.banks.iter().flatten().map(|b| b.memory()).sum();
        let gi = lib.genre_index("lofi").unwrap() as u16;
        let ctx = MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index("mothership_calm").unwrap() as u16, genre_a: gi, genre_b: gi, blend: 0.0, intensity: 0.3, duck_db: 0.0, ..Default::default() };
        let mut e = ScoreEngine::new(res, 48000.0, 20261114, ctx);
        let mut frames: Vec<[f32; 2]> = Vec::with_capacity(60 * 48000);
        let mut buf = [[0.0f32; 2]; 128];
        let (t0, mut most) = (std::time::Instant::now(), 0);
        for b in 0..(60.0 * 48000.0 / 128.0) as usize {
            if streamed {
                // Real time: never more than 20 ms ahead of the clock.
                let due = std::time::Duration::from_secs_f64(b as f64 * 128.0 / 48000.0);
                while t0.elapsed() + std::time::Duration::from_millis(20) < due {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            e.render(&mut buf);
            most = most.max(e.streaming());
            frames.extend_from_slice(&buf);
        }
        println!(
            "{}: the lo-fi score's recordings hold {:.1} MiB, the rings {:.1} MiB; {} notes streamed at most; rendered in {:.1} s",
            if streamed { "streamed" } else { "whole" },
            recordings as f64 / 1048576.0,
            bank.stream_memory() as f64 / 1048576.0,
            most,
            t0.elapsed().as_secs_f64()
        );
        if let Some(dir) = &wav_dir {
            let path = dir.join(if streamed { "st-full-keys-streamed.wav" } else { "st-full-keys-whole.wav" });
            humanity_music::render::write_wav(&path, &frames, 48000).expect("written");
            println!("  written: {}", path.display());
        }
        renders.push(frames);
    }
    let (a, b) = (&renders[0], &renders[1]);
    let diff = a.iter().zip(b).map(|(x, y)| (x[0] - y[0]).abs().max((x[1] - y[1]).abs())).fold(0.0f32, f32::max);
    let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
    let rms = |f: &[[f32; 2]]| (f.iter().map(|x| (x[0] * x[0] + x[1] * x[1]) as f64).sum::<f64>() / (2 * f.len()) as f64).sqrt();
    println!("before and after: {differing} of {} frames differ, the largest difference {diff:e}; RMS {:.6} and {:.6}", a.len(), rms(a), rms(b));
    assert_eq!(differing, 0, "the streamed render is the whole-file render, frame for frame");
}
