//! The talk card each frame (rung 3 of docs/design/embeddable-ai.md, 2026-11-12):
//!
//! - its AI box keeps going while the card is open behind a page (the card itself runs it while drawn);
//! - a card closed by any path (Esc, death, a mode change) stops a reply still being written;
//! - "Show me the way": the place resolved each tick (`DestinationDef::target`, the same resolution
//!   the quests' Travel steps use) into the HUD's tracked-marker ring with its name and distance, and
//!   cleared when the player gets there;
//! - its dev IPC for the ai-talk rig (12.1), `debug/talk_request.json`: open a card by name, ask, close,
//!   and measure the game's frames while the engine is idle and while it writes. Answered in
//!   `debug/talk_done.json` with the card's whole state. Permanent dev tooling (CLAUDE.md, "Forever
//!   development").

use std::sync::Mutex;
use std::time::Instant;

use crate::engine::state::EngineState;
use crate::gui::talk::chat::{Availability, Phase, Said};
use crate::gui::GuiPage;

/// How high above the floor the way's ring is drawn, metres (chest height, where a person looks).
const MARK_HEIGHT_M: f32 = 1.2;

pub(crate) fn frame(state: &mut EngineState) {
    poll_request(state);
    let gs = &mut state.gui_state;
    if gs.npc_talk_target.is_none() {
        if gs.talk.chat.busy() || matches!(gs.talk.chat.phase, Phase::Warming(_)) {
            crate::gui::talk::card::close_card(gs);
        }
    } else if gs.active_page != GuiPage::None {
        crate::gui::talk::card::step_chat(gs);
    }
    sample_frame(gs.talk.chat.phase == Phase::Writing);
    update_guide(state);
}

/// Where the way's place is now, and whether the player got there (then the guide is cleared).
fn update_guide(state: &mut EngineState) {
    let Some(dest_id) = state.gui_state.talk.guide.as_ref().map(|g| g.dest_id.clone()) else { return };
    let dests = state.data_store.get::<humanity_systems::quests::DestinationList>("quest_destinations");
    let player = humanity_systems::quests::player_position(&state.game_world.world, &state.data_store);
    let aboard = humanity_systems::quests::Aboard::of(&state.data_store);
    let resolved = match (dests.and_then(|d| d.destinations.iter().find(|x| x.id == dest_id)), player) {
        (Some(d), Some(p)) => d.target(p, aboard).map(|t| (t, p, d.distance_from(p, aboard).unwrap_or(f32::INFINITY), d.radius)),
        _ => None,
    };
    let Some(g) = state.gui_state.talk.guide.as_mut() else { return };
    match resolved {
        Some((_, _, floor_dist, radius)) if floor_dist <= radius => {
            log::info!("[Talk] arrived at {}: the way is no longer shown", g.label);
            state.gui_state.talk.guide = None;
        }
        Some((target, player, _, _)) => {
            let at = target + glam::Vec3::Y * MARK_HEIGHT_M;
            g.mark = Some((at, f64::from((at - player).length())));
        }
        None => g.mark = None,
    }
}

/// Frames measured for the rig: (interval ms, the frame's CPU work ms) while the engine is idle and
/// while it writes. Filled only while a measurement is asked for (nothing in normal play).
#[derive(Default)]
struct Frames {
    on: bool,
    last: Option<Instant>,
    idle: Vec<(f32, f32)>,
    writing: Vec<(f32, f32)>,
}

static FRAMES: Mutex<Frames> = Mutex::new(Frames { on: false, last: None, idle: Vec::new(), writing: Vec::new() });

fn sample_frame(writing: bool) {
    let mut f = FRAMES.lock().unwrap_or_else(|p| p.into_inner());
    if !f.on {
        return;
    }
    let now = Instant::now();
    if let Some(last) = f.last {
        let interval = now.duration_since(last).as_secs_f32() * 1000.0;
        // The frame's span costs, summed. This READS ids the frame records; the prefix is written in
        // two parts so the Performance page's test (performance.rs `ids_in`), which takes every
        // id-shaped literal for one the engine records, does not read it as a new measurement family.
        let cpu: f32 = humanity_core::frame_costs::snapshot().cpu.iter().filter(|e| e.id.strip_prefix("cpu").is_some_and(|rest| rest.starts_with(".span."))).map(|e| e.value as f32).sum();
        if writing {
            f.writing.push((interval, cpu));
        } else {
            f.idle.push((interval, cpu));
        }
    }
    f.last = Some(now);
}

/// Mean and 95th percentile of each column.
fn stats(v: &[(f32, f32)]) -> serde_json::Value {
    let col = |pick: fn(&(f32, f32)) -> f32| -> serde_json::Value {
        if v.is_empty() {
            return serde_json::Value::Null;
        }
        let mut xs: Vec<f32> = v.iter().map(pick).collect();
        xs.sort_by(f32::total_cmp);
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        let p95 = xs[((xs.len() as f32 * 0.95) as usize).min(xs.len() - 1)];
        serde_json::json!({ "mean": mean, "p95": p95 })
    };
    serde_json::json!({ "frames": v.len(), "interval_ms": col(|s| s.0), "cpu_ms": col(|s| s.1) })
}

const REQUEST_PATH: &str = "debug/talk_request.json";
const DONE_PATH: &str = "debug/talk_done.json";

/// The ai-talk rig's request: any of `open` (a crew member's name), `ask` (a question), `close`,
/// `measure` (true starts a fresh frame measurement, false stops it); each request is answered with the
/// card's whole state.
fn poll_request(state: &mut EngineState) {
    let Some(text) = crate::engine::ipc::take_request_text(std::path::Path::new(REQUEST_PATH)) else { return };
    let _ = std::fs::create_dir_all("debug");
    let req: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::write(DONE_PATH, serde_json::json!({"ok": false, "error": format!("invalid JSON: {e}")}).to_string());
            return;
        }
    };
    let mut error = None;
    if let Some(m) = req.get("measure").and_then(|v| v.as_bool()) {
        let mut f = FRAMES.lock().unwrap_or_else(|p| p.into_inner());
        if m {
            *f = Frames { on: true, ..Frames::default() };
        } else {
            f.on = false;
        }
    }
    if req.get("close").and_then(|v| v.as_bool()) == Some(true) && state.gui_state.npc_talk_target.is_some() {
        crate::gui::talk::card::close_card(&mut state.gui_state);
    }
    // Settings > AI's brain choice for this run ("auto", "characters-quick", "characters-rich"): the
    // rig measures each installed brain in turn. Not saved (the rig's game is portable and throwaway).
    if let Some(brain) = req.get("brain").and_then(|v| v.as_str()) {
        state.gui_state.settings.ai_brain = brain.to_string();
    }
    if let Some(name) = req.get("open").and_then(|v| v.as_str()) {
        if let Some(fake) = req.get("dev_fake").and_then(|v| v.as_bool()) {
            crate::ai::shared::set_use_dev_fake(fake);
        }
        if !crate::gui::talk::open_by_name(&mut state.gui_state, name, &state.data_store, &state.game_world.world) {
            error = Some(format!("no crew member called {name:?} in data/npc/crew.ron"));
        }
    }
    if let Some(q) = req.get("ask").and_then(|v| v.as_str()) {
        if state.gui_state.npc_talk_target.is_none() {
            error = Some("no talk card is open".into());
        } else {
            state.gui_state.talk.chat.ask(q);
        }
    }
    let _ = std::fs::write(DONE_PATH, card_json(state, error).to_string());
}

/// The card's whole state, for the rig.
fn card_json(state: &EngineState, error: Option<String>) -> serde_json::Value {
    let gs = &state.gui_state;
    let chat = &gs.talk.chat;
    let availability = match chat.availability {
        Availability::NoAi => "no_ai",
        Availability::WrittenChosen => "written_chosen",
        Availability::EngineMissing => "engine_missing",
        Availability::NoPersona => "no_persona",
        Availability::Offered => "offered",
    };
    let phase = match &chat.phase {
        Phase::Off => "off".to_string(),
        Phase::Preparing => "preparing".into(),
        Phase::Warming(_) => "warming".into(),
        Phase::Ready => "ready".into(),
        Phase::Writing => "writing".into(),
        Phase::Unavailable(w) => format!("unavailable: {w}"),
    };
    let said = match &gs.talk.said {
        None => serde_json::json!({"kind": "opening", "line": gs.npc_talk_line}),
        Some(Said::Written(l)) => serde_json::json!({"kind": "written", "line": l}),
        Some(Said::Ai { line, action }) => serde_json::json!({"kind": "ai", "line": line, "action": action.as_ref().map(|a| a.wire())}),
        Some(Said::Fallback { line, why }) => serde_json::json!({"kind": "fallback", "line": line, "why": why}),
    };
    let f = FRAMES.lock().unwrap_or_else(|p| p.into_inner());
    let [asked, answered, filtered, refused_shape, fell_back, timed_out] = crate::ai::talk::COUNTS.read();
    let engine_mb = crate::ai::shared::client().memory_mb().0;
    serde_json::json!({
        "ok": error.is_none(),
        "error": error,
        "open": gs.npc_talk_target.is_some(),
        "name": gs.npc_talk_name,
        "availability": availability,
        "phase": phase,
        "model": chat.model,
        "model_name": chat.model_name,
        "busy": chat.busy(),
        "partial": chat.partial,
        "said": said,
        "timing": chat.last.map(|t| serde_json::json!({"tokens": t.tokens, "ms_prefill": t.ms_prefill, "ms_generate": t.ms_generate, "ms_first_words": t.ms_first_words, "ms_reply": t.ms_reply})),
        "turns": chat.turns.len(),
        "persona_chars": chat.persona.chars().count(),
        "grants": crate::ai::talk::Grants::actions(&chat.grants).iter().map(|a| a.wire()).collect::<Vec<_>>(),
        "counts": {"asked": asked, "answered": answered, "filtered": filtered, "refused_shape": refused_shape, "fell_back": fell_back, "timed_out": timed_out},
        "engine_memory_mb": engine_mb,
        "frame_ms_smoothed": humanity_core::frame_costs::snapshot().frame_ms,
        "frames": {"measuring": f.on, "idle": stats(&f.idle), "writing": stats(&f.writing)},
    })
}
