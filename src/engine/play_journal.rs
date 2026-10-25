//! The play journal (docs/design/play-journal.md, rung 1): what happened while the game ran, by
//! kind, in small capped files a Claude session reads through the dev MCP (`dev_play_journal`,
//! `dev_play_marks` in scripts/mcp-server.js), and F8, Mark this moment, which writes a numbered
//! mark with a screenshot. Permanent dev tooling: never trim it as cleanup.
//!
//! - Where: a `journal` folder beside the logs folder (`<logs>/../journal`), so the operator's
//!   install and every rig's sandbox each have their own. `<kind>.jsonl`, one JSON line per event:
//!   `{"t": <unix ms>, "what": "<short words>", ...}`; marks' pictures in `marks/<n>.png`.
//! - Kinds: `data/dev/journal_kinds.json` (each kind's words, for the bridge and the Dev page).
//!   Rung 1 records `log` (every warning and error line, from the logger), `panic` and `mark`.
//! - Never in it: the words of a message, a key, a token, a recovery phrase, a file's contents.
//!   A `log` event is the same line `run.log` already holds, nothing more.
//! - Capped: a kind's file holds at most `KIND_CAP_BYTES`; past that the oldest half is dropped.
//!   The last `MARKS_KEPT` marks' pictures are kept.
//! - Cost: recording pushes onto a queue under a mutex, only when something happens; the writer
//!   thread appends the queue to the files once a second. Nothing is written on the render thread
//!   (a mark's picture goes through the renderer's own capture, as every screenshot does). A panic
//!   is written at once, since the process may be ending.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The most one kind's file holds; past it the oldest half is dropped.
pub const KIND_CAP_BYTES: u64 = 256 * 1024;
/// How many marks' pictures are kept.
pub const MARKS_KEPT: usize = 30;
/// The longest a recorded line's words may be; longer is cut (a log line can be long).
const WHAT_MAX_CHARS: usize = 600;

/// The journal's folder, once started.
static DIR: OnceLock<PathBuf> = OnceLock::new();
/// Events waiting for the writer: (kind, the JSON line).
static QUEUE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The journal's folder for a logs folder: its sibling `journal`.
pub fn dir_for(logs_dir: &Path) -> PathBuf {
    logs_dir.parent().map(|p| p.join("journal")).unwrap_or_else(|| logs_dir.join("journal"))
}

/// Start the journal beside `logs_dir`, and the writer thread that appends what was recorded once
/// a second. Called once at start, from lib.rs's logging set-up.
pub fn start(logs_dir: &Path) {
    let dir = dir_for(logs_dir);
    if std::fs::create_dir_all(dir.join("marks")).is_err() || DIR.set(dir).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("play-journal".into()).spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        flush();
    });
}

/// The journal's folder, when it is running.
pub fn dir() -> Option<&'static Path> {
    DIR.get().map(|p| p.as_path())
}

/// Record one event of `kind` (a row of data/dev/journal_kinds.json): `what` in a few words, and
/// `fields` for the detail. Written within a second. Never pass the words of a message, a key, a
/// token or a recovery phrase (the design's never-in-it list).
pub fn record(kind: &str, what: &str, fields: serde_json::Value) {
    if DIR.get().is_none() {
        return;
    }
    let line = event_line(now_ms(), what, fields);
    if let Ok(mut q) = QUEUE.lock() {
        q.push((kind.to_string(), line));
    }
}

/// One event as its JSON line: the time, the words (cut at `WHAT_MAX_CHARS`), and the fields.
pub fn event_line(t_ms: u64, what: &str, fields: serde_json::Value) -> String {
    let mut ev = serde_json::json!({ "t": t_ms, "what": cut(what) });
    if let (Some(obj), serde_json::Value::Object(more)) = (ev.as_object_mut(), fields) {
        for (k, v) in more {
            if k != "t" && k != "what" {
                obj.insert(k, v);
            }
        }
    }
    ev.to_string()
}

fn cut(words: &str) -> String {
    let words = words.trim();
    match words.char_indices().nth(WHAT_MAX_CHARS) {
        Some((i, _)) => format!("{}...", &words[..i]),
        None => words.to_string(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Write what is queued now: the writer thread once a second, and the panic hook at once.
pub fn flush() {
    let Some(dir) = DIR.get() else { return };
    let batch: Vec<(String, String)> = match QUEUE.lock() {
        Ok(mut q) => std::mem::take(&mut *q),
        Err(_) => return,
    };
    let mut by_kind: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (kind, line) in batch {
        by_kind.entry(kind).or_default().push(line);
    }
    for (kind, lines) in by_kind {
        append_capped(&dir.join(format!("{kind}.jsonl")), &lines, KIND_CAP_BYTES);
    }
}

/// Append `lines` to `path`; if it is then over `cap` bytes, keep only its newest half (whole
/// lines), so a long session never grows a file without bound and the newest is what stays.
pub fn append_capped(path: &Path, lines: &[String], cap: u64) {
    if lines.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let mut text = lines.join("\n");
        text.push('\n');
        let _ = f.write_all(text.as_bytes());
    }
    let over = std::fs::metadata(path).map(|m| m.len() > cap).unwrap_or(false);
    if !over {
        return;
    }
    let Ok(all) = std::fs::read_to_string(path) else { return };
    let keep_from = all.len().saturating_sub((cap / 2) as usize);
    // From the start of the first whole line at or after the halfway point.
    let start = all[keep_from..].find('\n').map(|i| keep_from + i + 1).unwrap_or(all.len());
    let _ = std::fs::write(path, &all[start..]);
}

/// A warning or error line as the logger wrote it (`[<time> WARN  <module>] <message>`): its level,
/// module and message, or None for any other line (info and below are not journal events).
pub fn log_event(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix('[')?;
    let close = rest.find(']')?;
    let mut head = rest[..close].split_whitespace();
    let _time = head.next()?;
    let level = head.next()?;
    if level != "WARN" && level != "ERROR" {
        return None;
    }
    let module = head.next().unwrap_or("");
    Some((level, module, rest[close + 1..].trim()))
}

/// The logger's feed (lib.rs `LogTee`): each warning or error line it writes becomes a `log`
/// event, the same words run.log holds.
pub fn note_log_bytes(buf: &[u8]) {
    if DIR.get().is_none() {
        return;
    }
    let Ok(text) = std::str::from_utf8(buf) else { return };
    for line in text.lines() {
        if let Some((level, module, message)) = log_event(line) {
            record("log", message, serde_json::json!({ "level": level, "module": module }));
        }
    }
}

/// The panic hook's: the panic's message and place, written at once.
pub fn note_panic(message: &str) {
    let first = message.lines().next().unwrap_or("");
    record("panic", first, serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }));
    flush();
}

/// The number the next mark gets: one past the highest picture in `marks/`, so it keeps counting
/// up across restarts.
pub fn next_mark_number(dir: &Path) -> u64 {
    let highest = std::fs::read_dir(dir.join("marks"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.path().file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok()))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    highest + 1
}

/// Keep the newest `keep` marks' pictures, deleting older ones.
pub fn prune_marks(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir.join("marks")) else { return };
    let mut numbered: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let n = p.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok())?;
            Some((n, p))
        })
        .collect();
    numbered.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    for (_, p) in numbered.into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// F8 or the Dev page asked for a mark (`GuiState::play_mark_request`): number it, save this frame
/// as its picture, record it with where the player is, and say so on screen. Called at the end
/// of the frame, beside the other captures (lib.rs).
#[cfg(feature = "native")]
pub(crate) fn take_mark(state: &mut crate::engine::state::EngineState, frame_texture: &wgpu::Texture) {
    if !std::mem::take(&mut state.gui_state.play_mark_request) {
        return;
    }
    let now = state.egui_ctx.input(|i| i.time);
    let Some(dir) = dir() else {
        state.gui_state.toast(crate::gui::i18n::t("dev.journal_not_running"), crate::gui::ToastKind::Error, now);
        return;
    };
    let n = next_mark_number(dir);
    let png = dir.join("marks").join(format!("{n}.png"));
    let picture = match state.renderer.capture_current_frame(frame_texture, &png) {
        Ok(()) => serde_json::json!(png.display().to_string()),
        Err(e) => {
            log::warn!("play journal: mark {n}'s picture was not saved: {e}");
            serde_json::Value::Null
        }
    };
    record(
        "mark",
        &format!("Mark {n}"),
        serde_json::json!({
            "n": n,
            "picture": picture,
            "page": format!("{:?}", state.gui_state.active_page),
            "storey": crate::engine::storey_probe::frame_json(state),
        }),
    );
    flush();
    prune_marks(dir, MARKS_KEPT);
    state.gui_state.toast(crate::gui::i18n::tf("dev.journal_marked", &[("n", &n)]), crate::gui::ToastKind::Info, now);
}

// ── Rung 3 (2026-10-24): the page, frame, world and net kinds the frame can see ──────────────

/// A frame counts as a hitch when it took at least this long ...
pub const HITCH_MIN_MS: f32 = 100.0;
/// ... and at least this many times the recent average.
pub const HITCH_TIMES_USUAL: f32 = 3.0;
/// At most one `frame` event this often, so a slow stretch is one line, not hundreds.
const HITCH_EVERY: std::time::Duration = std::time::Duration::from_secs(2);
/// A landing at least this fast is a fall worth a line (about a 3 m drop).
pub const FALL_MIN_M_S: f32 = 8.0;
/// How often the room the player is in is looked up.
const ROOM_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

/// What the frame watcher saw last, to record only changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seen {
    pub page: String,
    pub world_loaded: bool,
    pub aboard: bool,
    pub room: Option<String>,
    pub dead: bool,
    pub landings: u32,
    pub connected: Option<String>,
}

/// This frame's readings, as the watcher compares them with `Seen`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Now {
    pub page: String,
    pub world_loaded: bool,
    pub aboard: bool,
    /// None when not looked up this frame.
    pub room: Option<Option<String>>,
    pub dead: bool,
    pub landings: (u32, Option<f32>),
    /// The server's address when signed in, else None.
    pub connected: Option<String>,
    /// This frame's time and the recent average, in ms.
    pub frame_ms: Option<(f32, f32)>,
    /// A hitch may be recorded now (the last was long enough ago).
    pub hitch_allowed: bool,
}

/// The events one frame's readings make against what was seen before: (kind, words, fields).
/// Pure, so each rule is tested without a running game.
pub fn changes(seen: &mut Seen, now: &Now) -> Vec<(&'static str, String, serde_json::Value)> {
    use serde_json::json;
    let mut out = Vec::new();
    if now.page != seen.page {
        out.push(("page", format!("{} -> {}", seen.page, now.page), json!({ "from": seen.page, "to": now.page })));
        seen.page = now.page.clone();
    }
    if let Some((ms, usual)) = now.frame_ms {
        if now.hitch_allowed && ms >= HITCH_MIN_MS && ms >= HITCH_TIMES_USUAL * usual {
            out.push(("frame", format!("A {ms:.0} ms frame (usually {usual:.0} ms)"), json!({ "ms": ms, "usual_ms": usual, "page": now.page })));
        }
    }
    if now.world_loaded != seen.world_loaded {
        let words = if now.world_loaded { "Entered the world" } else { "Left the world" };
        out.push(("world", words.to_string(), json!({ "world_loaded": now.world_loaded })));
        seen.world_loaded = now.world_loaded;
    }
    if now.aboard != seen.aboard {
        let words = if now.aboard { "Went aboard the station" } else { "Left the station" };
        out.push(("world", words.to_string(), json!({ "aboard": now.aboard })));
        seen.aboard = now.aboard;
    }
    if let Some(room) = &now.room {
        if *room != seen.room {
            let words = match room {
                Some(r) => format!("In {r}"),
                None => "Out of every named room".to_string(),
            };
            out.push(("world", words, json!({ "room": room, "from": seen.room })));
            seen.room = room.clone();
        }
    }
    if now.dead != seen.dead {
        let words = if now.dead { "Died" } else { "Back to life" };
        out.push(("world", words.to_string(), json!({ "dead": now.dead })));
        seen.dead = now.dead;
    }
    let (landings, last_m_s) = now.landings;
    if landings > seen.landings {
        if let Some(speed) = last_m_s.filter(|s| *s >= FALL_MIN_M_S) {
            out.push(("world", format!("A fall: landed at {speed:.1} m/s"), json!({ "m_s": speed })));
        }
        seen.landings = landings;
    }
    if now.connected != seen.connected {
        let words = match (&seen.connected, &now.connected) {
            (_, Some(to)) => format!("Signed in to {to}"),
            (Some(from), None) => format!("Lost the connection to {from}"),
            (None, None) => String::new(),
        };
        out.push(("net", words, json!({ "server": now.connected.clone().or(seen.connected.clone()), "connected": now.connected.is_some() })));
        seen.connected = now.connected.clone();
    }
    out
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
#[cfg(feature = "native")]
pub(crate) fn note_frame(state: &crate::engine::state::EngineState) {
    if DIR.get().is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hos-play-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("marks")).unwrap();
        d
    }

    #[test]
    fn a_kind_past_its_cap_keeps_its_newest_half_in_whole_lines() {
        let d = scratch("cap");
        let path = d.join("log.jsonl");
        let lines: Vec<String> = (0..100).map(|i| format!("{{\"t\":{i},\"what\":\"line {i:03}\"}}")).collect();
        append_capped(&path, &lines, 1000);
        let kept = std::fs::read_to_string(&path).unwrap();
        assert!(kept.len() as u64 <= 500, "about half the cap: {} bytes", kept.len());
        assert!(kept.lines().all(|l| l.starts_with("{\"t\":") && l.ends_with("\"}")), "whole lines only");
        assert!(kept.lines().last().unwrap().contains("line 099"), "the newest stays");
        assert!(!kept.contains("line 000"), "the oldest goes");
        // Within the cap nothing is dropped.
        let small = d.join("small.jsonl");
        append_capped(&small, &lines[..3], 1000);
        assert_eq!(std::fs::read_to_string(&small).unwrap().lines().count(), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_warnings_and_errors_are_log_events() {
        let warn = "[2026-10-24T22:10:01.123Z WARN  humanity_engine::net::webrtc] WebRTC: recv_from error: x";
        assert_eq!(log_event(warn), Some(("WARN", "humanity_engine::net::webrtc", "WebRTC: recv_from error: x")));
        assert_eq!(log_event("[2026-10-24T22:10:01.123Z ERROR boot] no GPU").map(|e| e.0), Some("ERROR"));
        assert_eq!(log_event("[2026-10-24T22:10:01.123Z INFO  boot] starting"), None);
        assert_eq!(log_event("a continuation line of a long message"), None);
    }

    #[test]
    fn a_marks_number_keeps_counting_across_restarts_and_old_pictures_go() {
        let d = scratch("marks");
        assert_eq!(next_mark_number(&d), 1, "an empty journal starts at 1");
        for n in 1..=5u64 {
            std::fs::write(d.join("marks").join(format!("{n}.png")), b"png").unwrap();
        }
        assert_eq!(next_mark_number(&d), 6, "one past the highest, read from the folder");
        prune_marks(&d, 2);
        let mut left: Vec<String> = std::fs::read_dir(d.join("marks")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["4.png".to_string(), "5.png".to_string()], "the newest two kept");
        assert_eq!(next_mark_number(&d), 6, "numbers are never reused");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn kinds_of(events: &[(&'static str, String, serde_json::Value)]) -> Vec<(&'static str, String)> {
        events.iter().map(|(k, w, _)| (*k, w.clone())).collect()
    }

    #[test]
    fn only_what_changed_is_recorded_once() {
        let mut seen = Seen::default();
        let mut now = Now { page: "Chat".into(), world_loaded: true, connected: Some("https://public.guide".into()), ..Default::default() };
        let first = kinds_of(&changes(&mut seen, &now));
        assert_eq!(first, vec![
            ("page", " -> Chat".to_string()),
            ("world", "Entered the world".to_string()),
            ("net", "Signed in to https://public.guide".to_string()),
        ]);
        assert!(changes(&mut seen, &now).is_empty(), "the same frame again changes nothing");
        now.connected = None;
        now.page = "None".into();
        let second = kinds_of(&changes(&mut seen, &now));
        assert_eq!(second, vec![("page", "Chat -> None".to_string()), ("net", "Lost the connection to https://public.guide".to_string())]);
    }

    #[test]
    fn a_hitch_is_a_long_frame_against_the_usual_and_not_while_held_back() {
        let mut seen = Seen::default();
        let base = Now { page: "None".into(), ..Default::default() };
        let _ = changes(&mut seen, &base);
        let at = |ms: f32, usual: f32, allowed: bool| {
            let mut s = seen.clone();
            kinds_of(&changes(&mut s, &Now { frame_ms: Some((ms, usual)), hitch_allowed: allowed, ..base.clone() }))
        };
        assert_eq!(at(150.0, 16.0, true), vec![("frame", "A 150 ms frame (usually 16 ms)".to_string())]);
        assert!(at(90.0, 16.0, true).is_empty(), "under 100 ms is no hitch");
        assert!(at(150.0, 66.0, true).is_empty(), "a slow machine's usual frame is no hitch");
        assert!(at(150.0, 16.0, false).is_empty(), "at most one every two seconds");
    }

    #[test]
    fn rooms_deaths_and_falls_are_world_events() {
        let mut seen = Seen::default();
        let mut now = Now { page: "None".into(), ..Default::default() };
        let _ = changes(&mut seen, &now);
        now.room = Some(Some("Kitchen".into()));
        assert_eq!(kinds_of(&changes(&mut seen, &now)), vec![("world", "In Kitchen".to_string())]);
        now.room = None; // not looked up this frame: no change implied
        assert!(changes(&mut seen, &now).is_empty());
        now.landings = (1, Some(3.0));
        assert!(changes(&mut seen, &now).is_empty(), "a step down is no fall");
        now.landings = (2, Some(12.5));
        assert_eq!(kinds_of(&changes(&mut seen, &now)), vec![("world", "A fall: landed at 12.5 m/s".to_string())]);
        now.dead = true;
        assert_eq!(kinds_of(&changes(&mut seen, &now)), vec![("world", "Died".to_string())]);
    }

    #[test]
    fn an_event_line_carries_its_time_words_and_fields_and_long_words_are_cut() {
        let line = event_line(42, "hello", serde_json::json!({ "level": "WARN", "t": 7 }));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["t"], 42, "a field cannot replace the time");
        assert_eq!(v["what"], "hello");
        assert_eq!(v["level"], "WARN");
        let long = event_line(1, &"x".repeat(WHAT_MAX_CHARS + 50), serde_json::json!({}));
        let w: serde_json::Value = serde_json::from_str(&long).unwrap();
        assert_eq!(w["what"].as_str().unwrap().chars().count(), WHAT_MAX_CHARS + 3, "cut, with three dots");
    }

    #[test]
    fn every_kind_the_code_records_is_described_in_the_data() {
        let kinds: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/dev/journal_kinds.json")).unwrap(),
        )
        .unwrap();
        let listed: Vec<&str> = kinds["kinds"].as_array().unwrap().iter().filter_map(|k| k["id"].as_str()).collect();
        for kind in ["log", "panic", "mark", "page", "frame", "world", "net"] {
            assert!(listed.contains(&kind), "{kind} is in data/dev/journal_kinds.json");
        }
        let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine/play_journal.rs")).unwrap();
        for cap in source.split("record(\n        \"").skip(1).chain(source.split("record(\"").skip(1)) {
            let kind = cap.split('"').next().unwrap();
            assert!(listed.contains(&kind), "the recorded kind {kind} is listed");
        }
    }
}
