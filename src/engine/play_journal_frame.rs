//! The play journal's frame half (docs/design/play-journal.md): what only the running game can
//! give it. A mark asked for (F8, the Dev page, a rig) is numbered and recorded here at the end of
//! the frame, with that frame's picture through the renderer's own capture; and the frame watcher
//! reads the page, the world, the room, a death, a fall and the connection each frame and records
//! what changed. The store, the kinds and the rules of what counts as a change are the core's
//! (src/play_journal.rs), where every crate can record; these two work on the engine's state, so
//! they are the engine's (the crate split's rung 0, part B, 2026-10-31). Permanent dev tooling.

use std::sync::Mutex;

use crate::play_journal::{changes, dir, flush, next_mark_number, prune_marks, record, Now, Seen, MARKS_KEPT};

/// At most one `frame` event this often, so a slow stretch is one line, not hundreds.
const HITCH_EVERY: std::time::Duration = std::time::Duration::from_secs(2);
/// How often the room the player is in is looked up.
const ROOM_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

/// A mark was asked for (`GuiState::play_mark_request`: F8, the Dev page, or a rig through the
/// dev IPC): number it, save this frame as its picture when asked, record it with its label and
/// where the player is, and, for a mark by hand, say so on screen. Called at the end of the frame,
/// beside the other captures (lib.rs).
pub(crate) fn take_mark(state: &mut crate::engine::state::EngineState, frame_texture: &wgpu::Texture) {
    let Some(ask) = state.gui_state.play_mark_request.take() else {
        return;
    };
    let now = state.egui_ctx.input(|i| i.time);
    let Some(dir) = dir() else {
        state.gui_state.toast(crate::i18n::t("dev.journal_not_running"), crate::gui::ToastKind::Error, now);
        return;
    };
    let n = next_mark_number(dir);
    let png = dir.join("marks").join(format!("{n}.png"));
    let picture = if !ask.picture {
        serde_json::Value::Null
    } else {
        match state.renderer.capture_current_frame(frame_texture, &png) {
            Ok(()) => serde_json::json!(png.display().to_string()),
            Err(e) => {
                log::warn!("play journal: mark {n}'s picture was not saved: {e}");
                serde_json::Value::Null
            }
        }
    };
    let words = if ask.label.is_empty() { format!("Mark {n}") } else { format!("Mark {n}: {}", ask.label) };
    record(
        "mark",
        &words,
        serde_json::json!({
            "n": n,
            "label": ask.label,
            "picture": picture,
            "page": format!("{:?}", state.gui_state.active_page),
            "storey": crate::engine::storey_probe::frame_json(state),
        }),
    );
    flush();
    prune_marks(dir, MARKS_KEPT);
    // Only a mark by hand says so: a rig's note would stand in the picture it judges.
    if ask.picture && ask.label.is_empty() {
        state.gui_state.toast(crate::i18n::tf("dev.journal_marked", &[("n", &n)]), crate::gui::ToastKind::Info, now);
    }
}

/// What the watcher holds between frames.
struct Watch {
    seen: Seen,
    next_room: Option<std::time::Instant>,
    next_hitch: Option<std::time::Instant>,
}

/// Once a frame (lib.rs's main loop): the page, a hitch, the world, the room, a death, a fall and
/// the connection, recorded when they change. A frame with no change costs a few comparisons; the
/// room is looked up four times a second.
pub(crate) fn note_frame(state: &crate::engine::state::EngineState) {
    if dir().is_none() {
        return;
    }
    static WATCH: Mutex<Option<Watch>> = Mutex::new(None);
    let Ok(mut guard) = WATCH.lock() else { return };
    let w = guard.get_or_insert_with(|| Watch { seen: Seen::default(), next_room: None, next_hitch: None });
    let gs = &state.gui_state;
    let at = std::time::Instant::now();
    let room = if w.next_room.map_or(true, |t| at >= t) {
        w.next_room = Some(at + ROOM_EVERY);
        let eye = state.camera.position;
        Some(crate::gui::pages::hud::room_card_at(&gs.room_bounds, gs.ship_structure.as_ref(), &gs.keybinds, eye).map(|r| if r.display_name.is_empty() { r.id.clone() } else { r.display_name.clone() }))
    } else {
        None
    };
    let frame_ms = gs.frame_times.last().map(|&ms| {
        let usual = if gs.frame_times.is_empty() { ms } else { gs.frame_times.iter().sum::<f32>() / gs.frame_times.len() as f32 };
        (ms, usual)
    });
    let now = Now {
        page: format!("{:?}", gs.active_page),
        world_loaded: state.world_loaded,
        aboard: state.aboard_station,
        room,
        dead: gs.death_pack.note.is_some(),
        landings: state.controller.landings_taken(),
        connected: gs.server_connected.then(|| gs.server_url.clone()),
        frame_ms,
        hitch_allowed: w.next_hitch.map_or(true, |t| at >= t),
    };
    for (kind, words, fields) in changes(&mut w.seen, &now) {
        if kind == "frame" {
            w.next_hitch = Some(at + HITCH_EVERY);
        }
        record(kind, &words, fields);
    }
}
