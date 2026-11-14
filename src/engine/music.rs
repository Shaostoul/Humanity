//! The game's live music and the ship's hum, wired into the frame (docs/design/music.md, rungs 1 to
//! 4 and section 7b; the engine is crates/humanity-music).
//!
//! Each frame `frame` reads what the game already knows (in the world, on a page over it or on an
//! app page, aboard the ship or not, the zone and the room the camera stands in, the ship's hour, the
//! Settings > Music choices, the danger intensity, whether a chat video's sound is playing) and hands
//! the two playing sounds their `Copy` contexts through kira's latest-value channels: the score
//! (`ScoreSoundData`) and the bed (the hum and the tones, `BedSoundData`). Nothing here is networked.
//!
//! THE BUILD RUNS ON ITS OWN THREAD (BUG-270): reading the music's data, choosing and decoding the
//! recorded instruments (about 35 Ogg files, tens of megabytes decoded) and resolving them took the
//! game thread hundreds of milliseconds at world entry and again whenever the music came back after
//! a long page. Now [`build`] runs on a worker thread; the frame only starts it and, when it is done,
//! starts the score (silence until then: the score fades in from nothing anyway).
//!
//! A NEW SCORE TAKES OVER AT A PHRASE (BUG-266, BUG-267): when a build finishes while a score plays,
//! the new score starts at the playing one's next phrase boundary and the old one fades out over
//! about two seconds. A build is asked for when what is installed changes (an instrument pack
//! installed or removed: `instrument_packs::ui::installed_generation`), so a new pack is heard from
//! the next phrase, and, with the Dev page's "turn off all reuse" switch off, once every phrase, so
//! every phrase after the next is played from data and recordings read again, nothing kept: the
//! moods, genres and places files, the instruments and their samples (decoded again, the bank keeping
//! nothing). With reuse on, the bank keeps decoded recordings between builds until what is installed
//! changes or the music has been let go.
//!
//! OTHER SOUND WINS (music.md sections 8 and 12, rung 4; BUG-269): while a chat video's sound plays
//! (on the app's screen, docked, in its Full window, or on a screen in the world) the score sits
//! [`VIDEO_DUCK_DB`] under itself, going down in about 0.3 s and coming back in about 1.5 s
//! (`humanity_music::score::DUCK_DOWN_S`, `DUCK_UP_S`); the hum is not ducked. A page opened from the
//! world (Escape, or a key) keeps the world's music and hum playing [`OVER_WORLD_DB`] under, as if
//! through a door, until the person moves on to another page; the app's other pages keep their rule
//! (no music unless Settings > Music turns it on there). A sound not wanted for [`LET_GO_S`] is let go.
//!
//! The danger knob: `GuiState::music_dev.intensity` (the Dev page's slider, the `music_intensity`
//! showcase verb) and `breach` (the hum's alarm). Game systems (a meteor storm, an invasion) will
//! raise the same intensity; until then the knob is how a person or a rig reaches it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use humanity_music::kira_sound::{BedHandle, BedSoundData, ScoreHandle, ScoreSoundData};
use humanity_music::places::Place;
use humanity_music::{hum_place, BedContext, BedEngine, BedStatus, MusicContext, MusicLibrary, MusicSettings, MusicStatus, Resolved, ScoreEngine};

use crate::engine::state::EngineState;
use crate::gui::pages::dev_music::{MusicChoice, MusicReadout};
use humanity_music::favourite::Favourites;
use crate::gui::GuiPage;
use crate::instrument_packs::samples::SampleBank;

/// The rate the engines start at; kira's own rate takes over from its first block (kira_sound.rs).
const START_RATE: f32 = 48000.0;
/// A sound not wanted for this long is let go (it has long faded to silence by then).
pub(crate) const LET_GO_S: f32 = 30.0;
/// How far under itself the score sits while a chat video's sound plays, dB: the video is what the
/// person chose to hear, so it wins clearly, and the score stays faintly there rather than stopping
/// (music.md section 8 ducks a voice by about 9 dB; a video carries music of its own, so a little
/// more).
pub(crate) const VIDEO_DUCK_DB: f32 = -12.0;
/// How far under the world's music and hum sit on a page opened from the world: "as if heard through
/// a door" (music.md section 12, the escape menu, "ducked about 6 dB").
pub(crate) const OVER_WORLD_DB: f32 = -6.0;
/// With reuse off and no score playing, how often the music's data is read again for the hum.
const REREAD_S: f32 = 5.0;

/// Where the music is heard from this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hearing {
    /// In the 3D world.
    World,
    /// On the page Escape (or a key) opened from the world, the world still loaded behind it.
    OverWorld,
    /// On any other page (or the title screen).
    Page,
}

/// The hearing this frame, and the page kept as "the one opened from the world": `kept` is last
/// frame's, `was_in_world` whether last frame was in the world. Entering the world from a page, or
/// moving on to another page, ends the page-over-the-world.
pub(crate) fn hearing(was_in_world: bool, kept: Option<GuiPage>, world_loaded: bool, page: GuiPage) -> (Hearing, Option<GuiPage>) {
    if world_loaded && page == GuiPage::None {
        return (Hearing::World, None);
    }
    if world_loaded && page != GuiPage::MainMenu && (was_in_world || kept == Some(page)) {
        return (Hearing::OverWorld, Some(page));
    }
    (Hearing::Page, None)
}

/// What the frame hands the music: read from the engine's state by [`frame`], built by hand in the
/// tests.
pub(crate) struct Inputs {
    pub settings: MusicSettings,
    pub master: f32,
    pub music_volume: f32,
    pub hearing: Hearing,
    /// Aboard the mothership (the music and the hum play only there in the world).
    pub aboard: bool,
    /// A chat video's sound is playing somewhere.
    pub video_sound: bool,
    pub place: Here,
    pub intensity: f32,
    pub breach: bool,
    pub data_dir: PathBuf,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The Dev page's "turn off all reuse" switch (true: reuse is on, the default).
    pub reuse: bool,
    /// How many instrument installs and removals have finished this run.
    pub generation: u64,
    /// The Dev card's "start the music afresh".
    pub restart: bool,
    /// The Music page's choice while it is open with "Play here" on (docs/design/music-page.md
    /// section 4): its genre, mood, seed and tuning play in place of new pieces.
    pub page_choice: Option<MusicChoice>,
    /// The person's favourites (their own file, music/favourites.json): a mood's "my music" plays
    /// in that mood in place of new pieces (section 7).
    pub favourites: Arc<Favourites>,
    /// The Music page is open: the score measures its spectrum and role meters for it.
    pub analyse: bool,
}

/// What plays this frame and how loud: Settings' choices, where the music is heard from, and the duck.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Gains {
    pub music: bool,
    pub music_gain: f32,
    pub duck_db: f32,
    pub hum: bool,
    pub hum_gain: f32,
    pub tones: bool,
    pub tone_gain: f32,
}

/// The gains for `inp` (pure: the tests read it).
pub(crate) fn gains(inp: &Inputs) -> Gains {
    let s = &inp.settings;
    let worldly = matches!(inp.hearing, Hearing::World | Hearing::OverWorld);
    // On a page (the title screen included): when Settings > Music's one page switch plays it there,
    // from the app's start and all session (2026-11-11, the operator: "Play on the app pages as well.").
    let music = s.music_on && ((worldly && inp.aboard) || (inp.hearing == Hearing::Page && s.music_on_pages));
    let tones = s.tones_on && !music && (worldly || s.music_on_pages);
    let hum = s.ship_hum && worldly && inp.aboard;
    let master = inp.master.clamp(0.0, 1.0);
    let door = if inp.hearing == Hearing::OverWorld { OVER_WORLD_DB } else { 0.0 };
    let door_amp = 10f32.powf(door / 20.0);
    Gains {
        music,
        music_gain: if music { master * inp.music_volume.clamp(0.0, 1.0) } else { 0.0 },
        duck_db: door + if inp.video_sound { VIDEO_DUCK_DB } else { 0.0 },
        hum,
        // The hum is an ambience: it follows the Ambience Volume (and Master), never the Music
        // Volume, so turning the music off or down leaves it (rung 2; BUG-272 made the words say so).
        hum_gain: if hum { master * s.ambience_volume.clamp(0.0, 1.0) * s.ship_hum_level.clamp(0.0, 1.0) * door_amp } else { 0.0 },
        tones,
        tone_gain: if tones { master * s.tone_volume.clamp(0.0, 1.0) * door_amp } else { 0.0 },
    }
}

/// Where the camera stands, as the music reads it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Here {
    pub zone: String,
    pub room_id: String,
    pub room_name: String,
    pub room_volume_m3: Option<f32>,
    pub pos: (f64, f64, f64),
    pub hour: f32,
}

/// The zone purpose whose box holds `p` (ship metres), "" when none does.
pub(crate) fn zone_purpose_at(ship: Option<&humanity_ship::ship_structure::ShipStructure>, p: glam::Vec3) -> String {
    let Some(ship) = ship else { return String::new() };
    ship.zones
        .iter()
        .find(|z| {
            let o = z.origin_vec();
            let b = &z.body;
            p.x >= o.x && p.x <= o.x + b.width && p.y >= o.y - 0.5 && p.y <= o.y + b.height + 0.5 && p.z >= o.z && p.z <= o.z + b.depth
        })
        .map(|z| z.purpose.clone())
        .unwrap_or_default()
}

fn here(state: &EngineState) -> Here {
    let p = state.camera.position;
    let room = crate::gui::pages::hud::room_at(&state.gui_state.room_bounds, p);
    let hour = state
        .gui_state
        .game_time
        .as_ref()
        .map(|g| g.local_hour.unwrap_or(g.hour) * 24.0 / g.hours_per_day.max(1) as f32)
        .unwrap_or(12.0);
    Here {
        zone: zone_purpose_at(state.gui_state.ship_structure.as_ref(), p),
        room_id: room.map(|r| r.id.to_ascii_lowercase()).unwrap_or_default(),
        room_name: room.map(|r| r.display_name.to_ascii_lowercase()).unwrap_or_default(),
        room_volume_m3: room.map(|r| {
            let d = r.max - r.min;
            (d.x * d.y * d.z).max(0.0)
        }),
        pos: (p.x as f64, p.y as f64, p.z as f64),
        hour,
    }
}

/// The danger knob's showcase verbs: `"music_intensity":"0.8"` (0..1) and `"music_breach":"1"` (the
/// hum's alarm). Returns what it did, for the log. Permanent dev tooling (docs/ai/mcp.md: a rig
/// drives it through `game_showcase`).
pub(crate) fn showcase_verbs(state: &mut EngineState, intensity: Option<String>, breach: Option<String>) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(v) = intensity {
        match parse_intensity(&v) {
            Some(x) => {
                state.gui_state.music_dev.intensity = x;
                said.push(format!("music intensity -> {x:.2}"));
            }
            None => said.push(format!("music_intensity {v:?} is not a number from 0 to 1")),
        }
    }
    if let Some(v) = breach {
        let on = matches!(v.trim(), "1" | "true" | "on");
        state.gui_state.music_dev.breach = on;
        said.push(format!("music breach -> {on}"));
    }
    said
}

/// A showcase intensity: a number, clamped to 0..1.
pub(crate) fn parse_intensity(v: &str) -> Option<f32> {
    v.trim().parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 1.0))
}

/// What a build made.
pub(crate) struct Built {
    /// Every music data file, read now.
    pub lib: Arc<MusicLibrary>,
    /// The score's instruments with the installed recordings attached, resolved (None when only the
    /// hum was asked for).
    pub score: Option<Resolved>,
    /// What plays each part the music names, and what could not be decoded.
    pub plays: Vec<(String, crate::instrument_packs::score::PartPlays)>,
    pub problems: Vec<String>,
    /// Per genre: the parts it names that no installed recording plays (their words keys).
    pub stand_ins: Vec<Vec<String>>,
    /// How long it took on the worker, milliseconds.
    pub took_ms: f32,
}

/// Build the music from `data_dir`: read every data file, and with `with_score` choose the best
/// installed recording of each part the genres in use (`genres`, ids) can sound, decode it through
/// `bank`, level it and resolve the score. On the worker thread (the game thread never waits for it).
pub(crate) fn build(data_dir: &Path, a4: f32, with_score: bool, bank: &mut SampleBank, genres: &[String]) -> Result<Built, String> {
    let t0 = Instant::now();
    let lib = MusicLibrary::load(data_dir)?;
    let stand_ins = genre_stand_ins(data_dir, &lib);
    if !with_score {
        return Ok(Built { lib: Arc::new(lib), score: None, plays: Vec::new(), problems: Vec::new(), stand_ins, took_ms: t0.elapsed().as_secs_f32() * 1000.0 });
    }
    let mut with_parts = lib.clone();
    let attached = match crate::instrument_packs::catalog::Parts::load(data_dir) {
        Ok(parts) => {
            let installed = crate::instrument_packs::library::InstrumentLibrary::load(data_dir);
            let mut ids: Vec<usize> = genres.iter().filter_map(|id| lib.genre_index(id)).collect();
            if ids.is_empty() {
                ids.push(0);
            }
            crate::instrument_packs::score::attach(&mut with_parts, &installed, &parts, a4, bank, Some(&ids))
        }
        Err(e) => crate::instrument_packs::score::Attached { problems: vec![format!("data/music/parts.ron did not load, so every part is synthesised: {e}")], ..Default::default() },
    };
    let res = Resolved::new(&with_parts, &attached.samples)?;
    Ok(Built { lib: Arc::new(lib), score: Some(res), plays: attached.plays, problems: attached.problems, stand_ins, took_ms: t0.elapsed().as_secs_f32() * 1000.0 })
}

/// Settings > Music's genre choices: each genre's id and words, with its stand-ins (`stand_ins`, per
/// genre, from [`genre_stand_ins`]).
pub(crate) fn genre_choices(lib: &MusicLibrary, stand_ins: &[Vec<String>]) -> Vec<crate::gui::pages::dev_music::GenreChoice> {
    lib.genres
        .iter()
        .enumerate()
        .map(|(i, x)| crate::gui::pages::dev_music::GenreChoice {
            id: x.id.clone(),
            words: x.words.clone(),
            name: x.name.clone(),
            about_words: x.about_words.clone(),
            about: x.about.clone(),
            stand_ins: stand_ins.get(i).cloned().unwrap_or_default(),
        })
        .collect()
}

/// For each genre, the parts it names (its added roles' and its palette's) that no installed
/// recording plays, as their words keys, each once: Settings > Music says synthesised stand-ins play
/// them until a recording is installed (docs/design/music.md section 7a). Empty for a genre that
/// names no part (the default plays the mood's own).
pub(crate) fn genre_stand_ins(data_dir: &Path, lib: &MusicLibrary) -> Vec<Vec<String>> {
    let Ok(parts) = crate::instrument_packs::catalog::Parts::load(data_dir) else { return vec![Vec::new(); lib.genres.len()] };
    let installed = crate::instrument_packs::library::InstrumentLibrary::load(data_dir);
    lib.genres
        .iter()
        .map(|g| {
            let mut out: Vec<String> = Vec::new();
            for p in g.roles_add.iter().chain(g.leads.iter()).filter_map(|r| r.part.as_ref()).chain(g.palette.iter().filter_map(|e| e.part.as_ref())) {
                if matches!(installed.choose(p, &parts), Some(crate::instrument_packs::library::Choice::Sampled(_))) {
                    continue;
                }
                if let Some(w) = parts.part(p).map(|x| x.words.clone()) {
                    if !out.contains(&w) {
                        out.push(w);
                    }
                }
            }
            out
        })
        .collect()
}

/// A playing score, as the runtime drives it: kira's handle in the app, an offline renderer in tests.
/// Dropping it fades the score out.
pub(crate) trait ScorePlay {
    fn set_context(&mut self, ctx: MusicContext);
    fn status(&mut self) -> MusicStatus;
}

/// A playing bed (the hum and the tones).
pub(crate) trait BedPlay {
    fn set_context(&mut self, ctx: BedContext);
    fn status(&mut self) -> BedStatus;
}

/// Where the runtime's sounds play.
pub(crate) trait Mixer {
    fn play_score(&mut self, engine: ScoreEngine) -> Result<Box<dyn ScorePlay>, String>;
    fn play_bed(&mut self, engine: BedEngine) -> Result<Box<dyn BedPlay>, String>;
}

impl ScorePlay for ScoreHandle {
    fn set_context(&mut self, ctx: MusicContext) {
        ScoreHandle::set_context(self, ctx);
    }
    fn status(&mut self) -> MusicStatus {
        ScoreHandle::status(self)
    }
}

impl BedPlay for BedHandle {
    fn set_context(&mut self, ctx: BedContext) {
        BedHandle::set_context(self, ctx);
    }
    fn status(&mut self) -> BedStatus {
        BedHandle::status(self)
    }
}

impl<B: kira::backend::Backend> Mixer for humanity_core::audio::AudioManager<B> {
    fn play_score(&mut self, engine: ScoreEngine) -> Result<Box<dyn ScorePlay>, String> {
        Ok(Box::new(self.play_data(ScoreSoundData { engine })?))
    }
    fn play_bed(&mut self, engine: BedEngine) -> Result<Box<dyn BedPlay>, String> {
        Ok(Box::new(self.play_data(BedSoundData { engine })?))
    }
}

/// A build in flight on the worker thread: what it was asked for, and the bank lent to it.
type BuildResult = (Result<Built, String>, SampleBank);

/// Why a score is being built, which says when it takes over and with which seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Take {
    /// Nothing plays: it starts at once, a new piece.
    Start,
    /// The person chose another genre (or a place's genre took over): a new piece, at the playing
    /// score's next BAR line, crossfading (BUG-297: it waited for the next phrase, up to a minute).
    Genre,
    /// The piece has played its minutes (Settings > Music, "A new piece"): the next seed, at the
    /// playing score's next phrase.
    Piece,
    /// The same piece rebuilt (the data read again with reuse off, a pack installed): the same seed,
    /// carried on from the phrase it was at, at the next phrase.
    Refresh,
    /// What plays in place of new pieces changed (the Music page's choice: another seed, mood, key,
    /// tempo or lead voice; a mood's "my music" coming in or going): its piece, at the next BAR line,
    /// like a genre change (docs/design/music-page.md section 4).
    Choice,
}

/// A build on the worker: its thread, whether it builds a score, and the install generation and the
/// genres it was asked with, and why.
struct Building {
    handle: JoinHandle<BuildResult>,
    generation: u64,
    genres: Vec<String>,
    take: Take,
}

/// A built score waiting to start.
struct Ready {
    res: Resolved,
    /// The data it was built from (its mood and genre indices are this data's).
    lib: Arc<MusicLibrary>,
    generation: u64,
    genres: Vec<String>,
    /// The playing score's phrase and bar count when it arrived: it takes over after that phrase
    /// (or that bar, for a genre change).
    at_phrase: u64,
    at_bar: u64,
    take: Take,
}

/// The genres the music plays from (ids): the person's genre, or the place's livelier one where they
/// allow it, and a blend's second genre while the blend leans any way toward it.
pub(crate) fn genres_in_use(lib: Option<&MusicLibrary>, inp: &Inputs) -> Vec<String> {
    let s = &inp.settings;
    let mut a = s.genre.clone();
    if s.places_pick_genre {
        if let Some(lib) = lib {
            let p = Place { zone: &inp.place.zone, room_id: &inp.place.room_id, room_name: &inp.place.room_name, hour: inp.place.hour };
            if let Some(id) = lib.places.genre_for(&p) {
                a = id.to_string();
            }
        }
    }
    let mut out = vec![a];
    if !s.blend_with.is_empty() && s.blend > 0.0 && !out.contains(&s.blend_with) {
        out.push(s.blend_with.clone());
    }
    out
}

/// What the frame keeps between frames.
#[derive(Default)]
pub(crate) struct MusicRuntime {
    /// The newest music data (the hum's sources, the places, the genres for Settings).
    lib: Option<Arc<MusicLibrary>>,
    /// Per genre of `lib`: the parts no installed recording plays (Settings' stand-ins line).
    stand_ins: Vec<Vec<String>>,
    /// The data the playing score was built from (its mood and genre indices).
    score_lib: Option<Arc<MusicLibrary>>,
    load_error: Option<String>,
    score: Option<Box<dyn ScorePlay>>,
    bed: Option<Box<dyn BedPlay>>,
    seed: u64,
    score_idle_s: f32,
    bed_idle_s: f32,
    last: Option<Instant>,
    /// The page opened from the world, while it stays open.
    over_world: Option<GuiPage>,
    was_in_world: bool,
    /// The build on the worker.
    building: Option<Building>,
    /// The decoded recordings kept between builds (None while lent to the worker).
    bank: Option<SampleBank>,
    /// A built score waiting to start.
    ready: Option<Ready>,
    /// The install generation and the genres the playing score was built from.
    score_generation: u64,
    score_genres: Vec<String>,
    /// The playing score's phrase when the last rebuild was asked (reuse off: one per phrase).
    asked_at_phrase: Option<u64>,
    /// With reuse off and no score, seconds since the data was last read.
    since_read_s: f32,
    /// How many scores have started this run, and the last build's time on the worker.
    pieces: u64,
    build_ms: Option<f32>,
    /// Pieces begun this run (each new piece's seed is the session's plus this times the golden
    /// ratio's constant), the playing piece's seed, and how long it has played, seconds.
    piece_n: u64,
    piece_seed: u64,
    piece_s: f32,
    /// What the playing score plays in place of new pieces (the Music page's choice or a mood's "my
    /// music"; None: a new piece), its seed, its context when it began and a copy of what it plays
    /// from: the Music page writes the same piece down from them (MusicReadout::playing).
    score_choice: Option<MusicChoice>,
    score_seed: u64,
    score_ctx0: Option<MusicContext>,
    score_res: Option<Arc<Resolved>>,
    /// The Music page's export on its own thread.
    export: Option<JoinHandle<Result<PathBuf, String>>>,
}

/// Whether two "in place of new pieces" choices compose the same piece (both none, or the same seed,
/// genre, mood and composing half of the tuning).
fn same_choice(a: Option<&MusicChoice>, b: Option<&MusicChoice>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.composes_like(b),
        _ => false,
    }
}

/// The mood the game asks for now (an id): the mood an intensity band names when the danger
/// intensity is in it (tension, action), else the place's own. A mood's "my music" plays in it.
pub(crate) fn mood_now(lib: &MusicLibrary, inp: &Inputs) -> String {
    let band = lib.intensity.bands.iter().filter(|b| b.from <= inp.intensity).max_by(|a, b| a.from.total_cmp(&b.from));
    if let Some(m) = band.and_then(|b| b.mood.as_ref()) {
        return m.clone();
    }
    let p = Place { zone: &inp.place.zone, room_id: &inp.place.room_id, room_name: &inp.place.room_name, hour: inp.place.hour };
    lib.places.mood_for(&p).to_string()
}

/// What plays in place of new pieces now: the Music page's choice while it is open with Play here
/// on, else the favourite that is the music for the mood the game is in.
pub(crate) fn choice_now(lib: Option<&MusicLibrary>, inp: &Inputs) -> Option<MusicChoice> {
    if let Some(c) = &inp.page_choice {
        return Some(c.clone());
    }
    let lib = lib?;
    let fav = inp.favourites.for_mood(&mood_now(lib, inp))?;
    crate::gui::pages::music::my_music_choice(fav, lib)
}

impl MusicRuntime {
    /// Start a build on the worker thread (the bank goes with it and comes back with the result).
    fn start_build(&mut self, data_dir: &Path, a4: f32, with_score: bool, generation: u64, reuse: bool, genres: Vec<String>, take: Take) {
        if self.building.is_some() {
            return;
        }
        let mut bank = self.bank.take().unwrap_or_default();
        if !reuse {
            // Reuse off: nothing decoded before is handed out again (SampleBank::get also lets go
            // of everything on each request while the switch is off).
            bank.clear();
        }
        let dir = data_dir.to_path_buf();
        let asked = genres.clone();
        let handle = std::thread::Builder::new().name("music-build".into()).spawn(move || {
            let r = build(&dir, a4, with_score, &mut bank, &asked);
            (r, bank)
        });
        match handle {
            Ok(handle) => self.building = Some(Building { handle, generation, genres, take }),
            Err(e) => {
                log::warn!("[Music] the music's build thread did not start: {e}");
            }
        }
    }

    /// Take a finished build, if there is one.
    fn poll_build(&mut self, current_phrase: u64, current_bar: u64) {
        let finished = self.building.as_ref().is_some_and(|b| b.handle.is_finished());
        if !finished {
            return;
        }
        let Some(Building { handle, generation, genres, take }) = self.building.take() else { return };
        match handle.join() {
            Ok((result, bank)) => {
                self.bank = Some(bank);
                match result {
                    Ok(built) => {
                        for (part, plays) in &built.plays {
                            log::debug!("[Music] part {part}: {plays:?}");
                        }
                        for p in &built.problems {
                            log::warn!("[Music] {p}");
                        }
                        self.build_ms = Some(built.took_ms);
                        self.load_error = None;
                        let lib = built.lib.clone();
                        self.lib = Some(built.lib);
                        self.stand_ins = built.stand_ins;
                        if let Some(res) = built.score {
                            self.ready = Some(Ready { res, lib, generation, genres, at_phrase: current_phrase, at_bar: current_bar, take });
                        }
                    }
                    Err(e) => {
                        if self.load_error.as_deref() != Some(&e) {
                            log::warn!("[Music] the music data did not load, so no music plays: {e}");
                        }
                        self.load_error = Some(e);
                    }
                }
            }
            Err(_) => log::warn!("[Music] the music's build thread panicked"),
        }
    }

    /// One frame of the music, driven through `mixer` (None: no audio device, nothing plays, the
    /// Settings lists still come from the data). What it reports for the Dev card.
    pub(crate) fn step(&mut self, mixer: Option<&mut dyn Mixer>, inp: &Inputs) -> MusicReadout {
        if inp.restart {
            self.score = None;
            self.bed = None;
            self.ready = None;
            self.lib = None;
            self.score_lib = None;
        }
        let g = gains(inp);
        let mut readout = MusicReadout { device: mixer.is_some(), duck_db: g.duck_db, pieces: self.pieces, ..Default::default() };
        let st = self.score.as_mut().map(|h| h.status());
        let phrase = st.map(|s| s.phrase);
        let bar = st.map(|s| s.bars);
        self.poll_build(phrase.unwrap_or(0), bar.unwrap_or(0));
        let a4 = inp.settings.tuning.a4_hz();
        let want_bed = g.hum || g.tones;
        // What to build: a score when one is wanted (and a device plays it) and none plays or waits,
        // again when the genres in use change, when the piece has played its minutes, when what is
        // installed changed, and with reuse off once every phrase; otherwise the data alone, when
        // none has been read (Settings > Music lists its genres from it, music played or not; the hum
        // reads its sources from it), and with reuse off every few seconds.
        self.since_read_s += inp.dt;
        if self.score.is_some() && g.music {
            self.piece_s += inp.dt;
        }
        // In place of new pieces: the Music page's choice, or the mood's "my music" (music-page.md).
        let choice = choice_now(self.lib.as_deref(), inp);
        let genres = match &choice {
            Some(c) => vec![c.genre.clone()],
            None => genres_in_use(self.lib.as_deref(), inp),
        };
        // A score built for genres no longer in use (another was chosen while it was building, or
        // while it waited for its bar line) is not played: the build for the genres now starts at
        // once below (BUG-297: it played the stale score a phrase, then built again).
        if self.ready.as_ref().is_some_and(|r| r.genres != genres) {
            self.ready = None;
        }
        if self.building.is_none() && self.ready.is_none() {
            // The genres in use changed (a genre or a blend chosen in Settings, a place's
            // suggestion), the piece is due to change, or what is installed changed: the parts that
            // can sound, so the recordings to hold.
            let genre_changed = self.score.is_some() && self.score_genres != genres;
            let choice_changed = self.score.is_some() && !same_choice(self.score_choice.as_ref(), choice.as_ref());
            // A chosen piece plays on (a new piece every few minutes is for new pieces only).
            let piece_len_s = inp.settings.piece_minutes.max(0.0) * 60.0;
            let piece_due = choice.is_none() && self.score.is_some() && piece_len_s > 0.0 && self.piece_s >= piece_len_s;
            let installed = self.score.is_some() && self.score_generation != inp.generation;
            let next_phrase = !inp.reuse && phrase.is_some_and(|p| self.asked_at_phrase.is_none_or(|a| p > a));
            let unread = self.lib.is_none() && (self.load_error.is_none() || inp.restart);
            let take = if self.score.is_none() {
                Take::Start
            } else if genre_changed {
                Take::Genre
            } else if choice_changed {
                Take::Choice
            } else if piece_due {
                Take::Piece
            } else {
                Take::Refresh
            };
            if mixer.is_some() && ((g.music && self.score.is_none()) || genre_changed || choice_changed || piece_due || installed || next_phrase) {
                self.asked_at_phrase = phrase;
                self.since_read_s = 0.0;
                self.start_build(&inp.data_dir, a4, true, inp.generation, inp.reuse, genres.clone(), take);
            } else if unread || (!inp.reuse && self.score.is_none() && self.since_read_s >= REREAD_S) {
                self.since_read_s = 0.0;
                self.start_build(&inp.data_dir, a4, false, inp.generation, inp.reuse, genres.clone(), Take::Start);
            }
        }
        let Some(mixer) = mixer else {
            self.score = None;
            self.bed = None;
            self.ready = None;
            return self.readout_lists(readout);
        };
        if self.seed == 0 {
            self.seed = humanity_music::session_seed().max(1);
        }
        // A built score starts at once when none plays; a new genre at the playing one's next bar
        // line; anything else at its next phrase (and a new score's first phrase plays whole: it
        // is never taken over before its phrase 2, unless the person chose another genre).
        let take_over = match (&self.ready, phrase) {
            (Some(_), None) => g.music,
            (Some(r), Some(_)) if matches!(r.take, Take::Genre | Take::Choice) => bar.is_some_and(|b| b > r.at_bar),
            (Some(r), Some(p)) => p > r.at_phrase.max(1),
            _ => false,
        };
        if take_over {
            if let Some(Ready { res, lib, generation, genres, take, .. }) = self.ready.take() {
                let ctx = Self::context_of(&lib, inp, &g, a4, &genres, choice.as_ref());
                // A rebuild of the same piece keeps its seed and carries on from the phrase the old
                // score just began; a chosen piece (the Music page's, a mood's "my music") plays its
                // own seed; anything else is a new piece, the next seed.
                let resume = (take == Take::Refresh && self.score.is_some() && same_choice(self.score_choice.as_ref(), choice.as_ref())).then(|| st.map(|s| s.form_pos)).flatten();
                let seed = match (resume, &choice) {
                    (Some(_), _) => self.piece_seed,
                    (None, Some(c)) => c.seed,
                    (None, None) => self.seed.wrapping_add(self.piece_n.wrapping_mul(0x9e37_79b9)),
                };
                let res_copy = Arc::new(res.clone());
                let engine = match resume {
                    Some(at) => ScoreEngine::resume(res, START_RATE, seed, ctx, at),
                    None => ScoreEngine::new(res, START_RATE, seed, ctx),
                };
                match mixer.play_score(engine) {
                    Ok(h) => {
                        if self.score.is_some() {
                            log::info!("[Music] a new score takes over ({take:?}, seed {seed})");
                        } else {
                            log::info!("[Music] the score starts (seed {seed}, A4 = {a4} Hz)");
                        }
                        // The old one (if any) is dropped here: it fades out over about two seconds.
                        self.score = Some(h);
                        self.score_lib = Some(lib);
                        self.score_generation = generation;
                        self.score_genres = genres;
                        self.asked_at_phrase = None;
                        self.pieces += 1;
                        if resume.is_none() {
                            if choice.is_none() {
                                self.piece_n += 1;
                            }
                            self.piece_seed = seed;
                            self.piece_s = 0.0;
                            self.score_ctx0 = Some(ctx);
                        }
                        self.score_choice = choice.clone();
                        self.score_seed = seed;
                        self.score_res = Some(res_copy);
                    }
                    Err(e) => log::warn!("[Music] the score did not start: {e}"),
                }
            }
        } else if !g.music && self.score.is_none() {
            // Not wanted and nothing playing: a waiting score is not kept.
            self.ready = None;
        }
        self.score_idle_s = if g.music { 0.0 } else { self.score_idle_s + inp.dt };
        if self.score_idle_s > LET_GO_S && self.score.is_some() {
            self.score = None;
            self.score_lib = None;
            self.ready = None;
            // Nothing to play from for a while: the decoded recordings go too.
            if let Some(b) = self.bank.as_mut() {
                b.clear();
            }
        }
        let mut key_pc = 2i8;
        if let (Some(h), Some(lib)) = (self.score.as_mut(), self.score_lib.clone()) {
            // The playing score keeps the genres it was built for: a newly chosen genre arrives as a
            // new score at the next bar line, with its recordings, not as this score's stand-ins.
            let ctx = Self::context_of(&lib, inp, &g, a4, &self.score_genres, choice.as_ref());
            h.set_context(ctx);
            let st = h.status();
            key_pc = st.key_pc;
            readout.score = Some(st);
            if let (Some(res), Some(ctx0)) = (self.score_res.clone(), self.score_ctx0) {
                readout.playing = Some(crate::gui::pages::dev_music::Playing { choice: self.score_choice.clone(), seed: self.score_seed, ctx: ctx0, res, lib: lib.clone() });
            }
            readout.mood = lib.moods.get(st.mood as usize).map(|m| m.id.clone()).unwrap_or_default();
        }
        // The bed: the hum and the tones, from the newest data.
        if let Some(lib) = self.lib.clone() {
            let walls = u32::from(inp.place.room_volume_m3.is_some());
            let bed_ctx = BedContext {
                aboard: g.hum,
                place: hum_place(&lib.hum, inp.place.pos, walls, inp.place.room_volume_m3),
                key_pc,
                a4_hz: a4,
                gain: g.hum_gain,
                tone_hz: inp.settings.tone_hz.clamp(20.0, 4000.0),
                tone_gain: g.tone_gain,
                alarm: if inp.breach { ((inp.intensity - 0.35) / 0.4).clamp(0.0, 1.0) } else { 0.0 },
            };
            if want_bed && self.bed.is_none() {
                let engine = BedEngine::with_alarm(START_RATE, self.seed ^ 0xbed, bed_ctx, lib.hum.alarm);
                match mixer.play_bed(engine) {
                    Ok(h) => self.bed = Some(h),
                    Err(e) => log::warn!("[Music] the hum did not start: {e}"),
                }
            }
            if let Some(h) = self.bed.as_mut() {
                h.set_context(bed_ctx);
                readout.bed = Some(h.status());
            }
            readout.hum_level_db = bed_ctx.place.level_db;
            let p = Place { zone: &inp.place.zone, room_id: &inp.place.room_id, room_name: &inp.place.room_name, hour: inp.place.hour };
            readout.place_mood = lib.places.mood_for(&p).to_string();
        }
        self.bed_idle_s = if want_bed { 0.0 } else { self.bed_idle_s + inp.dt };
        if self.bed_idle_s > LET_GO_S {
            self.bed = None;
        }
        if readout.mood.is_empty() {
            readout.mood = readout.place_mood.clone();
        }
        readout.zone = inp.place.zone.clone();
        readout.seed = self.seed;
        readout.pieces = self.pieces;
        let piece_len_s = inp.settings.piece_minutes.max(0.0) * 60.0;
        readout.next_piece_s = (self.score.is_some() && piece_len_s > 0.0).then(|| (piece_len_s - self.piece_s).max(0.0));
        self.readout_lists(readout)
    }

    /// The score's context from `lib` (the data the score was built from: its mood and genre
    /// indices are that data's), playing `genres` (ids, as [`genres_in_use`] gives them: the genre,
    /// then a blend's second).
    /// A chosen piece (`choice`) brings its tuning, and the Music page's its own mood and intensity.
    fn context_of(lib: &MusicLibrary, inp: &Inputs, g: &Gains, a4: f32, genres: &[String], choice: Option<&MusicChoice>) -> MusicContext {
        let s = &inp.settings;
        let p = Place { zone: &inp.place.zone, room_id: &inp.place.room_id, room_name: &inp.place.room_name, hour: inp.place.hour };
        let chosen_mood = choice.and_then(|c| c.mood.as_deref()).and_then(|m| lib.mood_index(m));
        let mood = chosen_mood.unwrap_or_else(|| lib.mood_index(lib.places.mood_for(&p)).unwrap_or(0)) as u16;
        let intensity = choice.and_then(|c| c.intensity).unwrap_or(inp.intensity).clamp(0.0, 1.0);
        let tune = choice.map(|c| c.tune).unwrap_or_default();
        let genre_a = genres.first().and_then(|id| lib.genre_index(id)).unwrap_or(0) as u16;
        let (genre_b, blend) = match genres.get(1).and_then(|id| lib.genre_index(id)) {
            Some(i) if s.blend > 0.0 => (i as u16, s.blend.clamp(0.0, 1.0)),
            _ => (genre_a, 0.0),
        };
        MusicContext { gain: g.music_gain, a4_hz: a4, mood, genre_a, genre_b, blend, intensity, duck_db: g.duck_db, tune, analyse: inp.analyse }
    }

    /// The readout's lists (Settings > Music's genres and tones) and the build's state.
    fn readout_lists(&self, mut readout: MusicReadout) -> MusicReadout {
        if let Some(lib) = self.lib.as_ref() {
            readout.genres = genre_choices(lib, &self.stand_ins);
            readout.tones = lib.tones.clone();
            readout.piece_minutes = lib.piece_minutes.clone();
            readout.lib = Some(lib.clone());
        }
        readout.load_error = self.load_error.clone();
        readout.build_ms = self.build_ms;
        readout
    }

    /// Start the Music page's export on its own thread (one at a time): the music's data read and
    /// the favourite's genre's recordings decoded afresh (nothing borrowed from the playing score),
    /// then its piece rendered and written as it renders (render::export_favourite).
    pub(crate) fn start_export(&mut self, job: crate::gui::pages::music::ExportJob, data_dir: PathBuf, a4: f32) {
        if self.export.is_some() {
            return;
        }
        let spawned = std::thread::Builder::new().name("music-export".into()).spawn(move || -> Result<PathBuf, String> {
            let mut bank = SampleBank::default();
            let built = build(&data_dir, a4, true, &mut bank, std::slice::from_ref(&job.favourite.genre))?;
            let res = built.score.ok_or_else(|| "the music did not resolve".to_string())?;
            if let Some(dir) = job.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{} could not be made: {e}", dir.display()))?;
            }
            let clipped = humanity_music::render::export_favourite(res, &built.lib, &job.favourite, a4, job.minutes.max(0.1) as f64 * 60.0, &job.path)?;
            log::info!("[Music] exported {:?} ({} min, seed {}) to {} ({clipped} samples clipped)", job.favourite.name, job.minutes, job.favourite.seed, job.path.display());
            Ok(job.path)
        });
        match spawned {
            Ok(h) => self.export = Some(h),
            Err(e) => log::warn!("[Music] the export thread did not start: {e}"),
        }
    }

    /// The export's result once it is done.
    pub(crate) fn poll_export(&mut self) -> Option<Result<PathBuf, String>> {
        if !self.export.as_ref().is_some_and(|h| h.is_finished()) {
            return None;
        }
        let h = self.export.take()?;
        Some(h.join().unwrap_or_else(|_| Err("the export stopped unexpectedly".to_string())))
    }

    /// Whether a build is in flight (tests).
    #[cfg(test)]
    pub(crate) fn building(&self) -> bool {
        self.building.is_some()
    }

    /// Wait for the build in flight to finish (tests: the worker is a real thread).
    #[cfg(test)]
    pub(crate) fn wait_for_build(&self) {
        while self.building.as_ref().is_some_and(|b| !b.handle.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// Is any chat video's sound playing: on the app's screen (inline, docked or in its Full window) or
/// on a screen in the world?
fn chat_video_sound(state: &EngineState) -> bool {
    state.gui_state.chat_video.sounding() || state.screens.surfaces.iter().any(|s| s.core.chat_video_sounding())
}

/// One frame of the music (called from the audio block of lib.rs's frame).
pub(crate) fn frame(state: &mut EngineState) {
    let now = Instant::now();
    let dt = state.music.last.map(|t| now.duration_since(t).as_secs_f32()).unwrap_or(0.0).min(1.0);
    state.music.last = Some(now);
    let page = state.gui_state.active_page;
    let (h, kept) = hearing(state.music.was_in_world, state.music.over_world, state.world_loaded, page);
    state.music.over_world = kept;
    state.music.was_in_world = h == Hearing::World;
    // The person's favourites, read once (so a mood's "my music" plays without the Music page ever
    // being opened), and the page's choice while it is open.
    state.gui_state.music_page.load_favourites();
    let on_page = page == GuiPage::Music;
    let page_choice = if on_page { state.gui_state.music_page.choice(state.music.lib.as_deref(), &state.gui_state.settings.music.genre) } else { None };
    // The page's export: started on its own thread, and its result handed back to the page.
    if let Some(job) = state.gui_state.music_page.export_request.take() {
        state.music.start_export(job, state.data_dir.clone(), state.gui_state.settings.music.tuning.a4_hz());
    }
    if let Some(done) = state.music.poll_export() {
        state.gui_state.music_page.export_status = match done {
            Ok(path) => crate::gui::pages::music::ExportStatus::Done { path },
            Err(why) => crate::gui::pages::music::ExportStatus::Failed { why },
        };
    }
    let inputs = Inputs {
        settings: state.gui_state.settings.music.clone(),
        master: state.gui_state.settings.master_volume,
        music_volume: state.gui_state.settings.music_volume,
        hearing: h,
        aboard: state.world_loaded && crate::engine::home_plot::aboard(state) && state.aboard_station,
        video_sound: chat_video_sound(state),
        place: here(state),
        intensity: state.gui_state.music_dev.intensity,
        breach: state.gui_state.music_dev.breach,
        data_dir: state.data_dir.clone(),
        dt,
        reuse: humanity_core::reuse::enabled(),
        generation: crate::instrument_packs::ui::installed_generation(),
        restart: std::mem::take(&mut state.gui_state.music_dev.restart_requested),
        page_choice,
        favourites: state.gui_state.music_page.favourites.clone(),
        analyse: on_page,
    };
    let readout = {
        let EngineState { music, audio, .. } = &mut *state;
        music.step(audio.as_mut().map(|a| a as &mut dyn Mixer), &inputs)
    };
    state.gui_state.music_dev.readout = readout;
}

#[cfg(test)]
#[path = "music_tests.rs"]
mod runtime_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// THE DANGER KNOB'S SHOWCASE VERB reads a number and holds it to 0..1; anything else is refused
    /// with words, not taken as zero. RED first with `parse_intensity` as `v.parse().ok()` (no clamp):
    /// "2.5 is held to 1 (left: Some(2.5), right: Some(1.0))".
    #[test]
    fn the_music_intensity_verb_is_a_number_from_0_to_1() {
        assert_eq!(parse_intensity("0.8"), Some(0.8));
        assert_eq!(parse_intensity(" 0 "), Some(0.0));
        assert_eq!(parse_intensity("2.5"), Some(1.0), "2.5 is held to 1");
        assert_eq!(parse_intensity("-1"), Some(0.0));
        assert_eq!(parse_intensity("loud"), None);
        assert_eq!(parse_intensity("NaN"), None);
    }

    /// The zone the camera stands in comes from the shipped ship's own boxes: the Commons, First
    /// Street and the cabin block, each the purpose its file gives it.
    #[test]
    fn the_zone_purpose_comes_from_the_ship_file() {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/blueprints/ship_structure.ron");
        let ship: humanity_ship::ship_structure::ShipStructure = ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(zone_purpose_at(Some(&ship), glam::Vec3::new(90.0, 1.7, 50.0)), "commons");
        assert_eq!(zone_purpose_at(Some(&ship), glam::Vec3::new(70.0, 1.7, 600.0)), "street");
        assert_eq!(zone_purpose_at(Some(&ship), glam::Vec3::new(78.5, 1.5, 100.0)), "residence");
        assert_eq!(zone_purpose_at(Some(&ship), glam::Vec3::new(-500.0, 1.7, 50.0)), "");
        assert_eq!(zone_purpose_at(None, glam::Vec3::ZERO), "");
    }

    /// The shipped hum sources stand at today's stern, past p12 and the end of First Street (the
    /// mothership master plan's section 6.6), so the hum rises toward the stern and the Commons is
    /// the everywhere-hum.
    #[test]
    fn the_shipped_hum_rises_toward_the_stern() {
        let lib = MusicLibrary::load(&std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data")).unwrap();
        let at = |z: f64| hum_place(&lib.hum, (60.0, 1.7, z), 0, None).level_db;
        assert!(at(50.0) < 0.1, "the Commons: {}", at(50.0));
        assert!(at(1000.0) < at(1120.0) && at(1120.0) < at(1185.0), "rising toward the stern: {} {} {}", at(1000.0), at(1120.0), at(1185.0));
        assert!(at(1185.0) > 6.0, "loud at the street's end: {}", at(1185.0));
    }
}
