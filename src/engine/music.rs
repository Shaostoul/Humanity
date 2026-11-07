//! The game's live music and the ship's hum, wired into the frame (docs/design/music.md, rungs 1 to
//! 3 and section 7b; the engine is crates/humanity-music).
//!
//! Each frame `frame` reads what the game already knows (in the world or on a page, aboard the ship
//! or not, the zone and the room the camera stands in, the ship's hour, the Settings > Music choices,
//! the danger intensity) and hands the two playing sounds their `Copy` contexts through kira's
//! latest-value channels: the score (`ScoreSoundData`) and the bed (the hum and the tones,
//! `BedSoundData`). It starts a sound when it is wanted and the audio device is open, and lets it
//! fade out and go after 30 s of not being wanted (a page opened for a moment keeps the piece going
//! underneath, silent). Nothing here is networked, and nothing is kept across runs: the music's
//! data files are read afresh each time a sound starts.
//!
//! The danger knob: `GuiState::music_dev.intensity` (the Dev page's slider, the `music_intensity`
//! showcase verb) and `breach` (the hum's alarm). Game systems (a meteor storm, an invasion) will
//! raise the same intensity; until then the knob is how a person or a rig reaches it.

use std::sync::Arc;
use std::time::Instant;

use humanity_music::kira_sound::{BedHandle, BedSoundData, ScoreHandle, ScoreSoundData};
use humanity_music::places::Place;
use humanity_music::{hum_place, BedContext, BedEngine, MusicContext, MusicLibrary, Resolved, ScoreEngine};

use crate::engine::state::EngineState;
use crate::gui::pages::dev_music::MusicReadout;
use crate::gui::GuiPage;

/// The rate the engines start at; kira's own rate takes over from its first block (kira_sound.rs).
const START_RATE: f32 = 48000.0;
/// A sound not wanted for this long is let go (it has long faded to silence by then).
const LET_GO_S: f32 = 30.0;

/// What the frame keeps between frames.
#[derive(Default)]
pub(crate) struct MusicRuntime {
    lib: Option<Arc<MusicLibrary>>,
    load_error: Option<String>,
    score: Option<ScoreHandle>,
    bed: Option<BedHandle>,
    seed: u64,
    score_idle_s: f32,
    bed_idle_s: f32,
    last: Option<Instant>,
}

/// Where the camera stands, as the music reads it.
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

impl MusicRuntime {
    fn library(&mut self, data_dir: &std::path::Path, fresh: bool) -> Option<Arc<MusicLibrary>> {
        if fresh || self.lib.is_none() {
            match MusicLibrary::load(data_dir) {
                Ok(l) => {
                    self.lib = Some(Arc::new(l));
                    self.load_error = None;
                }
                Err(e) => {
                    if self.load_error.as_deref() != Some(&e) {
                        log::warn!("[Music] the music data did not load, so no music plays: {e}");
                    }
                    self.load_error = Some(e);
                    self.lib = None;
                }
            }
        }
        self.lib.clone()
    }
}

/// One frame of the music (called from the audio block of lib.rs's frame).
pub(crate) fn frame(state: &mut EngineState) {
    let now = Instant::now();
    let dt = state.music.last.map(|t| now.duration_since(t).as_secs_f32()).unwrap_or(0.0).min(1.0);
    state.music.last = Some(now);
    if std::mem::take(&mut state.gui_state.music_dev.restart_requested) {
        state.music.score = None;
        state.music.bed = None;
        state.music.lib = None;
    }
    let s = state.gui_state.settings.music.clone();
    let in_world = state.world_loaded && state.gui_state.active_page == GuiPage::None;
    let aboard = state.world_loaded && crate::engine::home_plot::aboard(state) && state.aboard_station;
    let music_here = s.music_on && ((in_world && aboard) || (!in_world && s.music_on_pages));
    let tones_here = s.tones_on && !music_here && (in_world || s.music_on_pages);
    let hum_here = s.ship_hum && in_world && aboard;
    let master = state.gui_state.settings.master_volume.clamp(0.0, 1.0);
    let music_gain = if music_here { master * state.gui_state.settings.music_volume.clamp(0.0, 1.0) } else { 0.0 };
    let hum_gain = if hum_here { master * s.ambience_volume.clamp(0.0, 1.0) * s.ship_hum_level.clamp(0.0, 1.0) } else { 0.0 };
    let tone_gain = if tones_here { master * s.tone_volume.clamp(0.0, 1.0) } else { 0.0 };
    let intensity = state.gui_state.music_dev.intensity.clamp(0.0, 1.0);
    let breach = state.gui_state.music_dev.breach;
    let place = here(state);
    let data_dir = state.data_dir.clone();
    let want_any = music_here || hum_here || tones_here || state.music.score.is_some() || state.music.bed.is_some();
    let mut readout = MusicReadout { device: state.audio.is_some(), ..Default::default() };
    if !want_any || state.audio.is_none() {
        let rt = &mut state.music;
        if let Some(lib) = rt.lib.as_ref() {
            readout.styles = lib.styles.iter().map(|x| (x.id.clone(), x.words.clone(), x.name.clone())).collect();
            readout.tones = lib.tones.clone();
        }
        readout.load_error = rt.load_error.clone();
        publish(state, readout);
        return;
    }
    let rt = &mut state.music;
    let fresh = (music_here && rt.score.is_none()) || ((hum_here || tones_here) && rt.bed.is_none());
    let Some(lib) = rt.library(&data_dir, fresh) else {
        readout.load_error = rt.load_error.clone();
        publish(state, readout);
        return;
    };
    if rt.seed == 0 {
        rt.seed = humanity_music::session_seed().max(1);
    }
    let a4 = s.tuning.a4_hz();
    let p = Place { zone: &place.zone, room_id: &place.room_id, room_name: &place.room_name, hour: place.hour };
    let mood = lib.mood_index(lib.places.mood_for(&p)).unwrap_or(0) as u16;
    let mut style_a = lib.style_index(&s.style).unwrap_or(0) as u16;
    if s.places_pick_style {
        if let Some(i) = lib.places.style_for(&p).and_then(|id| lib.style_index(id)) {
            style_a = i as u16;
        }
    }
    let (style_b, blend) = match lib.style_index(&s.blend_with) {
        Some(i) if !s.blend_with.is_empty() && s.blend > 0.0 => (i as u16, s.blend.clamp(0.0, 1.0)),
        _ => (style_a, 0.0),
    };
    let ctx = MusicContext { gain: music_gain, a4_hz: a4, mood, style_a, style_b, blend, intensity };
    let Some(audio) = state.audio.as_mut() else { return };
    audio.set_ambience_volume(f64::from(s.ambience_volume));
    // The score.
    if music_here && rt.score.is_none() {
        // The recorded instruments installed (the core set and any packs) play the parts the moods
        // name; the rest stay synthesised (instrument_packs::score). Decoded now, held by this score.
        let mut with_parts = (*lib).clone();
        let attached = match crate::instrument_packs::catalog::Parts::load(&data_dir) {
            Ok(parts) => {
                let installed = crate::instrument_packs::library::InstrumentLibrary::load(&data_dir);
                crate::instrument_packs::score::attach(&mut with_parts, &installed, &parts, a4)
            }
            Err(e) => {
                log::warn!("[Music] data/music/parts.ron did not load, so every part is synthesised: {e}");
                Default::default()
            }
        };
        for (part, plays) in &attached.plays {
            log::info!("[Music] part {part}: {plays:?}");
        }
        for p in &attached.problems {
            log::warn!("[Music] {p}");
        }
        match Resolved::new(&with_parts, &attached.samples) {
            Ok(res) => match audio.play_data(ScoreSoundData { engine: ScoreEngine::new(res, START_RATE, rt.seed, ctx) }) {
                Ok(h) => {
                    log::info!("[Music] the score starts (seed {}, A4 = {a4} Hz)", rt.seed);
                    rt.score = Some(h);
                }
                Err(e) => log::warn!("[Music] the score did not start: {e}"),
            },
            Err(e) => rt.load_error = Some(e),
        }
    }
    rt.score_idle_s = if music_here { 0.0 } else { rt.score_idle_s + dt };
    if rt.score_idle_s > LET_GO_S {
        rt.score = None;
    }
    let mut key_pc = 2i8;
    if let Some(h) = rt.score.as_mut() {
        h.set_context(ctx);
        let st = h.status();
        key_pc = st.key_pc;
        readout.score = Some(st);
    }
    // The bed: the hum and the tones.
    let walls = u32::from(place.room_volume_m3.is_some());
    let bed_ctx = BedContext {
        aboard: hum_here,
        place: hum_place(&lib.hum, place.pos, walls, place.room_volume_m3),
        key_pc,
        a4_hz: a4,
        gain: hum_gain,
        tone_hz: s.tone_hz.clamp(20.0, 4000.0),
        tone_gain,
        alarm: if breach { ((intensity - 0.35) / 0.4).clamp(0.0, 1.0) } else { 0.0 },
    };
    if (hum_here || tones_here) && rt.bed.is_none() {
        let engine = BedEngine::with_alarm(START_RATE, rt.seed ^ 0xbed, bed_ctx, lib.hum.alarm);
        match audio.play_data(BedSoundData { engine }) {
            Ok(h) => rt.bed = Some(h),
            Err(e) => log::warn!("[Music] the hum did not start: {e}"),
        }
    }
    rt.bed_idle_s = if hum_here || tones_here { 0.0 } else { rt.bed_idle_s + dt };
    if rt.bed_idle_s > LET_GO_S {
        rt.bed = None;
    }
    if let Some(h) = rt.bed.as_mut() {
        h.set_context(bed_ctx);
        readout.bed = Some(h.status());
    }
    readout.mood = lib.moods.get(readout.score.map(|s| s.mood as usize).unwrap_or(mood as usize)).map(|m| m.id.clone()).unwrap_or_default();
    readout.place_mood = lib.moods.get(mood as usize).map(|m| m.id.clone()).unwrap_or_default();
    readout.zone = place.zone.clone();
    readout.seed = rt.seed;
    readout.hum_level_db = bed_ctx.place.level_db;
    readout.styles = lib.styles.iter().map(|x| (x.id.clone(), x.words.clone(), x.name.clone())).collect();
    readout.tones = lib.tones.clone();
    readout.load_error = rt.load_error.clone();
    publish(state, readout);
}

fn publish(state: &mut EngineState, readout: MusicReadout) {
    state.gui_state.music_dev.readout = readout;
}

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
