//! Dev > Journal (2026-10-24, rung 2 of docs/design/play-journal.md): the play journal in the
//! app, so reading what the game recorded never needs a terminal or the dev MCP (the GUI-first
//! rule). Each kind's count and latest event, the marks (F8) with their pictures, Mark this
//! moment, Open the folder, and Clear the journal (asking first).
//!
//! The files are read on a worker thread at most every two seconds while this section is on
//! screen (engine/play_journal.rs writes them); the draw only shows the last reading. The kinds
//! come from data/dev/journal_kinds.json, their words from data/i18n/en.json
//! (`dev_journal.kind_<id>`), so a new kind is a data row, a words key and a record() call.
//! The website has no game to journal: its Dev Workbench lists Journal as the desktop app's.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{RichText, ScrollArea};

use crate::gui::i18n::{t, t_dyn, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// How often the files are read again while the section is on screen.
const READ_EVERY: Duration = Duration::from_secs(2);
/// How many marks are listed, newest first (the journal keeps the last 30 pictures).
const MARKS_SHOWN: usize = 30;

/// One kind as last read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KindRow {
    pub id: String,
    /// Recorded by this version of the game (data/dev/journal_kinds.json `built`).
    pub built: bool,
    /// Events its file holds now.
    pub count: usize,
    /// The newest event: its time (Unix ms) and words.
    pub latest: Option<(u64, String)>,
}

/// One mark as last read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarkRow {
    pub n: u64,
    pub t: u64,
    pub page: String,
    pub place: String,
    pub picture: Option<PathBuf>,
}

/// Everything the section shows, as last read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub kinds: Vec<KindRow>,
    /// Newest first.
    pub marks: Vec<MarkRow>,
}

/// The kinds listed in data/dev/journal_kinds.json: (id, built).
pub fn listed_kinds(json: &str) -> Vec<(String, bool)> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("kinds").and_then(|k| k.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|k| Some((k.get("id")?.as_str()?.to_string(), k.get("built").and_then(|b| b.as_bool()).unwrap_or(false))))
        .collect()
}

/// Read a journal folder: each listed kind's count and newest event, and the marks.
pub fn read_snapshot(dir: &Path, kinds: &[(String, bool)]) -> Snapshot {
    let events = |kind: &str| -> Vec<serde_json::Value> {
        std::fs::read_to_string(dir.join(format!("{kind}.jsonl")))
            .map(|text| text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
            .unwrap_or_default()
    };
    let rows = kinds
        .iter()
        .map(|(id, built)| {
            let ev = events(id);
            let latest = ev.last().map(|e| (e["t"].as_u64().unwrap_or(0), e["what"].as_str().unwrap_or("").to_string()));
            KindRow { id: id.clone(), built: *built, count: ev.len(), latest }
        })
        .collect();
    let mut marks: Vec<MarkRow> = events("mark")
        .iter()
        .map(|m| MarkRow {
            n: m["n"].as_u64().unwrap_or(0),
            t: m["t"].as_u64().unwrap_or(0),
            page: m["page"].as_str().unwrap_or("").to_string(),
            place: m["storey"]["room"].as_str().or_else(|| m["storey"]["name"].as_str()).unwrap_or("").to_string(),
            picture: m["picture"].as_str().map(PathBuf::from).filter(|p| p.exists()),
        })
        .collect();
    marks.reverse();
    marks.truncate(MARKS_SHOWN);
    Snapshot { kinds: rows, marks }
}

/// Delete the journal: every kind's file and every mark's picture. The folder stays, and the
/// game's writer starts the files again with the next event.
pub fn clear(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("jsonl") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("marks")) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The last reading, when it was asked for, and whether a read is running.
struct Reader {
    snapshot: Snapshot,
    asked: Option<Instant>,
    busy: bool,
}

fn reader() -> &'static Mutex<Reader> {
    static R: OnceLock<Mutex<Reader>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Reader { snapshot: Snapshot::default(), asked: None, busy: false }))
}

/// The last reading, starting a fresh one on a worker thread when it is older than `READ_EVERY`.
fn refresh(ctx: &egui::Context, dir: &Path) -> Snapshot {
    let Ok(mut r) = reader().lock() else { return Snapshot::default() };
    let due = r.asked.map_or(true, |at| at.elapsed() >= READ_EVERY);
    if due && !r.busy {
        r.busy = true;
        r.asked = Some(Instant::now());
        let dir = dir.to_path_buf();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let json = std::fs::read_to_string(crate::data_dir().join("dev").join("journal_kinds.json")).unwrap_or_default();
            let snap = read_snapshot(&dir, &listed_kinds(&json));
            if let Ok(mut r) = reader().lock() {
                r.snapshot = snap;
                r.busy = false;
            }
            ctx.request_repaint();
        });
    }
    r.snapshot.clone()
}

/// Open a file or folder with the computer's own program for it.
fn open_with_system(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// What a button on the section asked for, applied after the draw.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Mark,
    OpenFolder,
    OpenPicture(PathBuf),
    Clear,
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let dir = crate::engine::play_journal::dir().map(Path::to_path_buf);
    let snap = dir.as_deref().map(|d| refresh(ctx, d)).unwrap_or_default();
    let mut act = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                act = draw_section(ui, theme, dir.as_deref(), &snap);
            });
        });
    match act {
        Some(Act::Mark) => state.play_mark_request = true,
        Some(Act::OpenFolder) => {
            if let Some(d) = dir.as_deref() {
                open_with_system(d);
            }
        }
        Some(Act::OpenPicture(p)) => open_with_system(&p),
        Some(Act::Clear) => {
            if let Some(d) = dir.as_deref() {
                clear(d);
                if let Ok(mut r) = reader().lock() {
                    r.asked = None; // read again at once
                }
                let now = ctx.input(|i| i.time);
                state.toast(t("dev_journal.cleared"), crate::gui::ToastKind::Success, now);
            }
        }
        None => {}
    }
    ctx.request_repaint_after(READ_EVERY);
}

/// The whole section for one reading. Pure: the tests draw fixtures through it.
pub fn draw_section(ui: &mut egui::Ui, theme: &Theme, dir: Option<&Path>, snap: &Snapshot) -> Option<Act> {
    let mut act = None;
    let small = |s: String| RichText::new(s).size(theme.font_size_small);
    ui.label(RichText::new(t("dev_journal.title")).size(theme.font_size_title).color(theme.text_primary()));
    ui.label(small(t("dev_journal.about").to_string()).color(theme.text_secondary()));
    ui.label(small(t("dev_journal.never_in").to_string()).color(theme.text_muted()));
    ui.add_space(theme.spacing_sm);
    let Some(dir) = dir else {
        widgets::card(ui, theme, |ui| {
            ui.label(small(t("dev_journal.not_running").to_string()).color(theme.text_muted()));
        });
        return None;
    };
    ui.label(small(tf("dev_journal.folder", &[("path", &dir.display())])).color(theme.text_secondary()));
    ui.horizontal(|ui| {
        if widgets::Button::primary(t("dev_journal.mark_now")).hint("dev_journal.mark_now_hint").show(ui, theme) {
            act = Some(Act::Mark);
        }
        if widgets::Button::secondary(t("dev_journal.open_folder")).hint("dev_journal.open_folder_hint").show(ui, theme) {
            act = Some(Act::OpenFolder);
        }
        let asking = egui::Id::new("dev_journal_clear_asking");
        let mut ask = ui.data(|d| d.get_temp::<bool>(asking)).unwrap_or(false);
        if !ask {
            if widgets::Button::danger(t("dev_journal.clear")).hint("dev_journal.clear_hint").show(ui, theme) {
                ask = true;
            }
        } else {
            ui.label(small(t("dev_journal.clear_confirm").to_string()).color(theme.warning()));
            if widgets::Button::danger(t("dev_journal.clear_yes")).hint("dev_journal.clear_yes_hint").show(ui, theme) {
                act = Some(Act::Clear);
                ask = false;
            }
            if widgets::Button::secondary(t("dev_journal.clear_no")).hint("dev_journal.clear_no_hint").show(ui, theme) {
                ask = false;
            }
        }
        ui.data_mut(|d| d.insert_temp(asking, ask));
    });
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev_journal.kinds")).size(theme.font_size_body).strong().color(theme.text_primary()));
        egui::Grid::new("dev_journal_kinds").num_columns(4).spacing([theme.spacing_md, theme.spacing_xs]).striped(true).show(ui, |ui| {
            for k in &snap.kinds {
                ui.label(small(k.id.clone()).strong().color(theme.text_primary()));
                ui.label(small(t_dyn(&format!("dev_journal.kind_{}", k.id)).to_string()).color(theme.text_secondary()));
                let count = if !k.built {
                    t("dev_journal.kind_not_built").to_string()
                } else if k.count == 0 {
                    t("dev_journal.kind_empty").to_string()
                } else {
                    tf("dev_journal.kind_count", &[("n", &k.count)])
                };
                ui.label(small(count).color(theme.text_muted()));
                let latest = k
                    .latest
                    .as_ref()
                    .map(|(t_ms, what)| format!("{}  {}", crate::gui::pages::chat::format_timestamp(*t_ms), what))
                    .unwrap_or_default();
                ui.label(small(latest).color(theme.text_secondary()));
                ui.end_row();
            }
        });
    });
    ui.add_space(theme.spacing_sm);

    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev_journal.marks")).size(theme.font_size_body).strong().color(theme.text_primary()));
        if snap.marks.is_empty() {
            ui.label(small(t("dev_journal.no_marks").to_string()).color(theme.text_muted()));
        }
        for m in &snap.marks {
            ui.horizontal(|ui| {
                let place = [m.page.as_str(), m.place.as_str()].iter().filter(|s| !s.is_empty() && **s != "None").copied().collect::<Vec<_>>().join(", ");
                let line = tf(
                    "dev_journal.mark_row",
                    &[("n", &m.n), ("when", &crate::gui::pages::chat::format_timestamp(m.t)), ("where", &place)],
                );
                ui.label(small(line).color(theme.text_primary()));
                match &m.picture {
                    Some(p) => {
                        if widgets::Button::ghost(t("dev_journal.open_picture")).hint("dev_journal.open_picture_hint").show(ui, theme) {
                            act = Some(Act::OpenPicture(p.clone()));
                        }
                    }
                    None => {
                        ui.label(small(t("dev_journal.no_picture").to_string()).color(theme.text_muted()));
                    }
                }
            });
        }
    });
    act
}

#[cfg(test)]
#[path = "dev_journal_tests.rs"]
mod tests;
