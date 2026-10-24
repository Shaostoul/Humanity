//! The desktop Notes and Calendar kept between sessions (2026-10-24).
//!
//! Until now neither was saved anywhere: `GuiState.notes` and `GuiState.cal_events` lived only in
//! memory, so every note and event vanished when the app closed. They are kept in one small file,
//! `personal.json`, beside `config.json` (`AppConfig::config_path`, so a portable sandbox keeps
//! its own), written crash-safely (`persistence::write_atomic`: a crash leaves the whole old file or
//! the whole new one).
//!
//! The file is read the first time the Notes or Calendar page is drawn and written by those pages
//! only when its contents changed. Test builds never read or write it, so a page snapshot's sample
//! notes and events can never reach a person's real file.

use crate::gui::{GuiCalendarEvent, GuiNote, GuiState};
use std::sync::Mutex;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Personal {
    #[serde(default)]
    notes: Vec<NoteRec>,
    #[serde(default)]
    calendar: Vec<EventRec>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NoteRec {
    id: u64,
    title: String,
    content: String,
    modified: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EventRec {
    title: String,
    year: i32,
    month: u32,
    day: u32,
    time: String,
    #[serde(default)]
    description: String,
    /// The event's colour as RGBA.
    color: [u8; 4],
}

/// The last bytes written (or read), so an unchanged frame writes nothing.
static LAST: Mutex<Option<String>> = Mutex::new(None);

fn path() -> std::path::PathBuf {
    crate::config::AppConfig::config_path().with_file_name("personal.json")
}

fn snapshot(state: &GuiState) -> Personal {
    Personal {
        notes: state
            .notes
            .iter()
            .map(|n| NoteRec { id: n.id, title: n.title.clone(), content: n.content.clone(), modified: n.modified })
            .collect(),
        calendar: state
            .cal_events
            .iter()
            .map(|e| EventRec {
                title: e.title.clone(),
                year: e.year,
                month: e.month,
                day: e.day,
                time: e.time.clone(),
                description: e.description.clone(),
                color: e.color.to_array(),
            })
            .collect(),
    }
}

/// Put what a file held into the state.
fn apply(state: &mut GuiState, p: Personal) {
    state.notes = p
        .notes
        .into_iter()
        .map(|n| GuiNote { id: n.id, title: n.title, content: n.content, modified: n.modified })
        .collect();
    state.cal_events = p
        .calendar
        .into_iter()
        .map(|e| GuiCalendarEvent {
            title: e.title,
            year: e.year,
            month: e.month,
            day: e.day,
            time: e.time,
            description: e.description,
            color: egui::Color32::from_rgba_unmultiplied(e.color[0], e.color[1], e.color[2], e.color[3]), // theme-exempt: a colour the person chose for their own event, read back from their file
        })
        .collect();
}

/// Read `personal.json` into the state once per run, the first time Notes or Calendar is drawn.
/// What is already in memory (nothing, in a normal start) is kept if the file cannot be read.
pub fn ensure_loaded(state: &mut GuiState) {
    if cfg!(test) || state.personal_loaded {
        return;
    }
    state.personal_loaded = true;
    match std::fs::read(path()) {
        Ok(bytes) => match serde_json::from_slice::<Personal>(&bytes) {
            Ok(p) => apply(state, p),
            Err(e) => log::warn!("personal.json could not be read ({e}); notes and calendar start empty and the file is kept until something changes"),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("personal.json could not be opened: {e}"),
    }
    if let Ok(s) = serde_json::to_string_pretty(&snapshot(state)) {
        *LAST.lock().unwrap() = Some(s);
    }
}

/// Write `personal.json` when the notes or events differ from what was last written.
pub fn save_if_changed(state: &GuiState) {
    if cfg!(test) || !state.personal_loaded {
        return;
    }
    let Ok(s) = serde_json::to_string_pretty(&snapshot(state)) else { return };
    let mut last = LAST.lock().unwrap();
    if last.as_deref() == Some(s.as_str()) {
        return;
    }
    match crate::persistence::write_atomic(&path(), s.as_bytes()) {
        Ok(()) => *last = Some(s),
        Err(e) => log::warn!("personal.json could not be written: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note and an event come back from the file exactly as they went in: the title, the
    /// words, the time, the description and the colour. Seen red 2026-10-24 with the event's
    /// description left out of the saved record ("the description comes back").
    #[test]
    fn notes_and_events_come_back_from_the_file_as_they_went_in() {
        let mut a = GuiState::default();
        a.notes.push(GuiNote { id: 7, title: "Seeds".into(), content: "Order tomato seed by March.".into(), modified: 1_761_300_000 });
        a.cal_events.push(GuiCalendarEvent {
            title: "Harvest lettuce".into(),
            year: 2026,
            month: 6,
            day: 21,
            time: "09:00".into(),
            description: "Outer leaves first.".into(),
            color: egui::Color32::from_rgb(80, 180, 80), // theme-exempt: test fixture
        });
        let json = serde_json::to_string_pretty(&snapshot(&a)).expect("the state saves");
        let mut b = GuiState::default();
        apply(&mut b, serde_json::from_str(&json).expect("the file reads back"));
        assert_eq!(b.notes.len(), 1);
        assert_eq!((b.notes[0].id, b.notes[0].title.as_str(), b.notes[0].content.as_str(), b.notes[0].modified), (7, "Seeds", "Order tomato seed by March.", 1_761_300_000));
        assert_eq!(b.cal_events.len(), 1);
        let e = &b.cal_events[0];
        assert_eq!((e.title.as_str(), e.year, e.month, e.day, e.time.as_str()), ("Harvest lettuce", 2026, 6, 21, "09:00"));
        assert_eq!(e.description, "Outer leaves first.", "the description comes back");
        assert_eq!(e.color, egui::Color32::from_rgb(80, 180, 80), "the colour comes back"); // theme-exempt: test fixture
    }

    /// A file from before a field existed still reads (a missing description is empty).
    #[test]
    fn an_event_saved_without_a_description_reads_as_empty() {
        let p: Personal = serde_json::from_str(r#"{"calendar":[{"title":"t","year":2026,"month":1,"day":2,"time":"x","color":[1,2,3,255]}]}"#).expect("reads");
        assert_eq!(p.calendar[0].description, "");
        assert!(p.notes.is_empty(), "a file with no notes list has none");
    }
}
