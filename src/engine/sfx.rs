//! The game's one-shot sounds, played on the volume slider each belongs to (BUG-265, 2026-11-07).
//!
//! Three queues reach the mixer here, once a frame (lib.rs's audio stage) or once a tick while the
//! window is hidden (tray_window.rs):
//!
//! - `EngineState::pending_sfx`: sounds raised by actions (a door, a pickup, a gather), each a
//!   catalogue id with a fallback path. Each plays on ITS CATALOGUE BUS (`data/sounds.toml`'s
//!   `bus`), the same bus Settings > Audio > Sound levels' Play uses, so Play and the game agree.
//!   Before, every one of them played on the sound-effects bus whatever its catalogue said: with
//!   the UI Volume at 0 a notice's chime still sounded and its Play was silent.
//! - `EngineState::pending_notice_sfx`: a notice's sound (engine/notify.rs). It plays on the
//!   NOTIFICATIONS bus whatever file the person picked for it (the "soft" and "pop" choices are the
//!   menu-open and item-pickup files, catalogued on the interface bus): a notice is a notification,
//!   so turning the clicks down never silences one (docs/design/loudness.md, rung 4).
//! - the ECS systems' `"sfx_events"` channel (construction and crafting finishing), catalogued the
//!   same way as the first.
//!
//! The volumes the sliders hold reach the mixer here too ([`sync_volumes`]), whenever one moves.

use std::path::Path;

use humanity_core::audio::sounds::{asset_path, SoundCatalog};
use humanity_core::audio::AudioManager;

use crate::engine::state::EngineState;

/// The bus a notice's sound plays on: Settings > Audio's Notifications volume.
pub(crate) const NOTICE_BUS: &str = "notifications";

/// Play the catalogued sound `id` once (its file from the catalogue, else `fallback`, under the
/// app's assets folder; its authored volume), on `bus` when given, else on its catalogue bus. The
/// amplitude it played at (master x bus x volume x its loudness level x the person's level for
/// it), for the tests: the same number Settings' Play would give for the same bus.
pub(crate) fn play_one<B: kira::backend::Backend>(
    audio: &mut AudioManager<B>,
    catalog: &SoundCatalog,
    assets: &Path,
    id: &str,
    fallback: &str,
    bus: Option<&str>,
) -> Result<f64, String> {
    let path = asset_path(assets, &catalog.path_or(id, fallback));
    let vol = catalog.volume_or_default(id);
    audio.play_sound_bus(&path, vol, bus.unwrap_or_else(|| catalog.bus_or_default(id)))
}

/// Play everything queued since the last call: the actions' sounds on their catalogue buses, the
/// notices' on the notifications bus, and the ECS systems' channel. With no audio device the queues
/// are emptied and nothing plays.
pub(crate) fn play_queued(state: &mut EngineState) {
    let system_sfx: Vec<(String, String)> = state
        .data_store
        .get::<std::sync::Mutex<Vec<(String, String)>>>("sfx_events")
        .and_then(|m| m.lock().ok().map(|mut e| e.drain(..).collect()))
        .unwrap_or_default();
    let Some(audio) = state.audio.as_mut() else {
        state.pending_sfx.clear();
        state.pending_notice_sfx.clear();
        return;
    };
    let assets = humanity_core::storage::assets_dir();
    let catalog = &state.sound_catalog;
    let actions = state.pending_sfx.drain(..).map(|(id, fallback)| (id.to_string(), fallback.to_string(), None));
    let notices = state.pending_notice_sfx.drain(..).map(|(id, fallback)| (id.to_string(), fallback.to_string(), Some(NOTICE_BUS)));
    let systems = system_sfx.into_iter().map(|(id, fallback)| (id, fallback, None));
    for (id, fallback, bus) in actions.chain(notices).chain(systems) {
        if let Err(e) = play_one(audio, catalog, &assets, &id, &fallback, bus) {
            // Once per run is plenty: a missing file fails every time it is asked for.
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| log::warn!("[Audio] sfx {id}: {e}"));
        }
    }
}

/// The interface click (a real widget activation this frame, lib.rs decides), on its catalogue bus
/// (the interface volume).
pub(crate) fn play_click(state: &mut EngineState) {
    let Some(audio) = state.audio.as_mut() else { return };
    let assets = humanity_core::storage::assets_dir();
    if let Err(e) = play_one(audio, &state.sound_catalog, &assets, "sfx.button_click", "audio/ui/button_click.ogg", None) {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| log::warn!("[Audio] ui click: {e}"));
    }
}

/// The Settings > Audio volumes as the mixer takes them: master, music, sound effects, interface,
/// notifications, ambience (Settings > Music's Ambience Volume).
pub(crate) fn wanted_volumes(state: &EngineState) -> [f32; 6] {
    let s = &state.gui_state.settings;
    [s.master_volume, s.music_volume, s.sfx_volume, s.ui_volume, s.notifications_volume, s.music.ambience_volume]
}

/// Push the Settings volumes into the mixer when one moved since the last push (honest sliders:
/// kira is touched only on a change).
pub(crate) fn sync_volumes(state: &mut EngineState) {
    let v = wanted_volumes(state);
    if v == state.audio_volumes_applied {
        return;
    }
    let Some(audio) = state.audio.as_mut() else { return };
    audio.set_master_volume(f64::from(v[0]));
    audio.set_music_volume(f64::from(v[1]));
    audio.set_sfx_volume(f64::from(v[2]));
    audio.set_ui_volume(f64::from(v[3]));
    audio.set_notifications_volume(f64::from(v[4]));
    audio.set_ambience_volume(f64::from(v[5]));
    state.audio_volumes_applied = v;
}

#[cfg(test)]
#[path = "sfx_tests.rs"]
mod tests;
