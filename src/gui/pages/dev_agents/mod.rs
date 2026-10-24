//! Dev > Claude: what Claude is doing (operator, 2026-10-22, the integrations between
//! HumanityOS and Claude Code). One read-only panel in the Dev tab that answers, from this
//! computer's own files, the questions `just brief`, `just check-delivery`, `git worktree list`
//! and `node scripts/agent-status.js` answer in a terminal:
//! - which agent worktrees exist, each one's branch, last commit and uncommitted files, and
//!   whether the agent holding it is still running, and the agents' handoff notes
//!   (`.claude/handoff/*.md` in the checkout and in each worktree: what each one wrote down as
//!   done, verified and next, so its work survives a restart);
//! - the newest three journal decisions and the top of PRIORITIES TIER 0;
//! - the checkout's version against the newest GitHub release, and whether the taskbar exe holds
//!   this tree's code (check-delivery's verdict, ported in `delivery.rs`);
//! - the share guard, the disk guard, the rigs' boot lock, and the build slot (which compiles
//!   of our crate run now and which wait their turn for memory, docs/ai/builds.md).
//!
//! NEVER ON THE FRAME. Every read runs on a short-lived background thread, started only while
//! the panel is on screen, at four rates: the small files every 3 s, git every 10 s, the
//! delivery check every minute (it hashes the compiled-in sources and reads two exes), GitHub
//! every ten minutes. Each job writes its part of the report and asks for a repaint; the frame
//! only copies the report and draws it. Nothing is kept between readings: each job reads afresh.
//!
//! READ-ONLY. Nothing here writes a file, takes a lock or starts anything but git and one
//! GitHub request. git runs with optional locks off, so the panel never holds an agent's index.
//!
//! The words are in `data/i18n/en.json` under `dev_claude` (the website's Dev page reads the
//! same keys). The web mirror is the Claude tab of `web/pages/dev.html`, which shows what a
//! website can reach (GitHub and the journal the site serves) and says plainly what it cannot.

pub mod delivery;
pub mod read;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, ScrollArea};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use read::{LockState, Reading, Worktree};

/// How often each kind of reading is refreshed while the panel is on screen.
const FILES_EVERY: Duration = Duration::from_secs(3);
const GIT_EVERY: Duration = Duration::from_secs(10);
const DELIVERY_EVERY: Duration = Duration::from_secs(60);
const RELEASE_EVERY: Duration = Duration::from_secs(600);

/// The newest journal decisions and TIER 0 headings shown.
const DECISIONS: usize = 3;
const HEADINGS: usize = 6;

/// The journal under the checkout's data folder (the website reads the copy it serves at
/// /data/coordination/orchestrator_state.json: tests/page_parity_lint.rs holds the two together).
const JOURNAL: &str = "coordination/orchestrator_state.json";
/// The words, under the data folder (the website fetches the same file).
const STRINGS: &str = "i18n/en.json";

/// Everything the panel shows, as last read.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// The main checkout the panel reads (Absent when this exe is not in one).
    pub checkout: Reading<std::path::PathBuf>,
    pub worktrees: Reading<Vec<Worktree>>,
    /// The agents' handoff notes, from the checkout and every worktree, newest first.
    pub notes: Reading<Vec<read::Note>>,
    pub decisions: Reading<Vec<read::Decision>>,
    pub tier0: Reading<Vec<String>>,
    /// Cargo.toml's version.
    pub version: Reading<String>,
    pub release: Reading<read::Release>,
    pub delivery: Reading<delivery::Delivery>,
    pub share_guard: Reading<read::ShareGuard>,
    pub disk_guard: Reading<read::DiskGuard>,
    pub boot_lock: Reading<LockState>,
    /// The machine-wide build slot: who compiles our crate now, who waits for memory.
    pub build_slot: Reading<Vec<read::SlotEntry>>,
    /// When each kind was last read (Unix seconds).
    pub files_at: Option<i64>,
    pub git_at: Option<i64>,
    pub delivery_at: Option<i64>,
    pub release_at: Option<i64>,
}

// ── The words ────────────────────────────────────────────────────────────────────────────────

/// The `dev_claude` strings: the data folder's en.json first, or, when that is missing,
/// unreadable or older than these words, the copy built into the exe, said in the log the way
/// every data loader says it (`note_builtin_copy`; scripts/lib/compiled-in.js checks this shape).
fn strings() -> &'static serde_json::Value {
    static S: OnceLock<serde_json::Value> = OnceLock::new();
    S.get_or_init(|| {
        let parse = |t: &str| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok();
        let section = |v: serde_json::Value| v.get("dev_claude").cloned();
        let disk = std::fs::read_to_string(crate::data_dir().join(STRINGS)).ok().and_then(|t| parse(&t)).and_then(section);
        disk.unwrap_or_else(|| {
            crate::embedded_data::note_builtin_copy("i18n/en.json", "the data folder's copy has no dev_claude words");
            parse(crate::embedded_data::I18N_EN_JSON).and_then(section).unwrap_or(serde_json::Value::Null)
        })
    })
}

/// One string by key; the key itself when it is missing (so a gap shows instead of hiding).
pub(crate) fn tr(key: &str) -> String {
    strings().get(key).and_then(|v| v.as_str()).unwrap_or(key).to_string()
}

/// A string with its `{{name}}` places filled (the web i18n's placeholder form).
pub(crate) fn trf(key: &str, vars: &[(&str, String)]) -> String {
    let mut s = tr(key);
    for (name, value) in vars {
        s = s.replace(&format!("{{{{{name}}}}}"), value);
    }
    s
}

/// "40 s ago", "12 min ago", "3 h ago", "2 days ago".
pub(crate) fn ago(now: i64, then: i64) -> String {
    let secs = now - then;
    if secs < 5 {
        tr("just_now")
    } else if secs < 120 {
        trf("ago_s", &[("n", secs.to_string())])
    } else if secs < 2 * 3600 {
        trf("ago_m", &[("n", (secs / 60).to_string())])
    } else if secs < 2 * 86_400 {
        trf("ago_h", &[("n", (secs / 3600).to_string())])
    } else {
        trf("ago_d", &[("n", (secs / 86_400).to_string())])
    }
}

/// Every key the panel uses (the test checks each one is in en.json).
pub(crate) const KEYS: &[&str] = &[
    "title", "intro", "no_checkout", "checkout", "waiting", "absent", "unreadable", "just_now", "ago_s", "ago_m",
    "ago_h", "ago_d", "worktrees", "worktrees_hint", "main_checkout", "detached", "uncommitted", "clean",
    "uncommitted_unknown", "held_running", "held_gone", "held", "not_held", "no_commit", "prunable", "no_worktrees",
    "journal", "journal_hint", "tier0", "tier0_hint", "version", "this_checkout", "latest_release", "published",
    "in_sync", "ahead", "behind", "taskbar", "delivered", "delivered_trailing", "not_delivered", "not_built",
    "taskbar_missing", "taskbar_no_stamp", "built_missing", "built_stale", "delivery_hint", "this_pc",
    "share_guard", "share_running", "share_stopped", "share_never", "disk_guard", "disk_ran", "disk_nothing",
    "disk_cleared", "disk_skipped", "disk_late", "disk_never", "boot_lock", "lock_free", "lock_held",
    "lock_stale", "lock_unreadable", "read_times", "git_missing", "notes", "notes_hint", "no_notes", "note_line",
    "build_slot", "slot_none", "slot_idle", "slot_summary", "slot_compiling", "slot_waiting",
];

/// The newest handoff notes shown.
const NOTES: usize = 8;
/// The build slot's compiles and waiters shown, one line each.
const SLOT_LINES: usize = 8;

// ── The background jobs ──────────────────────────────────────────────────────────────────────

/// One kind of reading: when it last started, and whether it is running now.
#[derive(Default)]
struct Job {
    started: Option<Instant>,
    busy: Arc<AtomicBool>,
}

impl Job {
    /// Claim the job when it is due and not already running.
    fn due(&mut self, every: Duration) -> Option<Arc<AtomicBool>> {
        if self.busy.load(Ordering::Acquire) || self.started.is_some_and(|t| t.elapsed() < every) {
            return None;
        }
        self.started = Some(Instant::now());
        self.busy.store(true, Ordering::Release);
        Some(self.busy.clone())
    }
}

#[derive(Default)]
struct Panel {
    report: Arc<Mutex<Report>>,
    files: Job,
    git: Job,
    delivery: Job,
    release: Job,
    /// A fixed report (the snapshot): never refreshed.
    pinned: bool,
}

fn panel() -> &'static Mutex<Panel> {
    static P: OnceLock<Mutex<Panel>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(Panel::default()))
}

/// What a job hands back: how to store what it read into the report.
type Store = Box<dyn FnOnce(&mut Report) + Send>;

/// Run `work` on a thread of its own, then store what it read and ask for a repaint.
fn spawn(ctx: &egui::Context, busy: Arc<AtomicBool>, report: Arc<Mutex<Report>>, name: &str, work: impl FnOnce() -> Store + Send + 'static) {
    let ctx = ctx.clone();
    let flag = busy.clone();
    let started = std::thread::Builder::new().name(format!("dev-claude-{name}")).spawn(move || {
        let store = work();
        if let Ok(mut r) = report.lock() {
            store(&mut r);
        }
        busy.store(false, Ordering::Release);
        ctx.request_repaint();
    });
    if started.is_err() {
        // No thread: free the job so the next due time tries again.
        flag.store(false, Ordering::Release);
    }
}

/// Where to look for the checkout: the data folder, the exe's folder, the working folder.
fn starts() -> Vec<std::path::PathBuf> {
    let mut out = vec![crate::data_dir()];
    if let Ok(exe) = std::env::current_exe() {
        out.extend(exe.parent().map(|p| p.to_path_buf()));
    }
    out.extend(std::env::current_dir().ok());
    // Made absolute without resolving links: canonicalize gives \\?\ paths on Windows, which
    // read badly on the panel and which a child process cannot always take as its folder.
    out.into_iter().map(|p| std::path::absolute(&p).unwrap_or(p)).collect()
}

/// Start every job that is due and hand back a copy of the report to draw.
fn refresh(ctx: &egui::Context) -> Report {
    let Ok(mut p) = panel().lock() else { return Report::default() };
    if !p.pinned {
        let report = p.report.clone();
        if let Some(busy) = p.files.due(FILES_EVERY) {
            spawn(ctx, busy, report.clone(), "files", || -> Store {
                let checkout = read::find_checkout(&starts());
                let files = checkout.as_deref().map(read_files);
                let lock_file = read::boot_lock_path().map(|p| read::read_text(&p)).unwrap_or(Reading::Absent("LOCALAPPDATA".into()));
                let running = read::running_pids();
                let boot_lock = read::lock_state(&lock_file, running.as_ref());
                let guards = read::local_state_dir();
                let share = guards.as_ref().map(|d| read::read_text(&d.join("share-guard.json")).then(|t| read::parse_share_guard(&t)));
                let disk = guards.as_ref().map(|d| read::read_text(&d.join("disk-guard-last.json")).then(|t| read::parse_disk_guard(&t)));
                let slot = read::build_slot_dir().map(|d| read::read_build_slot(&d, running.as_ref()));
                Box::new(move |r: &mut Report| {
                    r.checkout = match checkout {
                        Some(c) => Reading::Found(c),
                        None => Reading::Absent("Cargo.toml".into()),
                    };
                    if let Some((decisions, tier0, version)) = files {
                        r.decisions = decisions;
                        r.tier0 = tier0;
                        r.version = version;
                    }
                    r.boot_lock = boot_lock;
                    r.share_guard = share.unwrap_or(Reading::Absent("LOCALAPPDATA".into()));
                    r.disk_guard = disk.unwrap_or(Reading::Absent("LOCALAPPDATA".into()));
                    r.build_slot = slot.unwrap_or(Reading::Absent("LOCALAPPDATA".into()));
                    r.files_at = Some(read::now_unix());
                })
            });
        }
        // The jobs below need the checkout the files job found first.
        let checkout = report.lock().ok().and_then(|r| r.checkout.found().cloned());
        if let Some(checkout) = checkout {
            if let Some(busy) = p.git.due(GIT_EVERY) {
                let c = checkout.clone();
                spawn(ctx, busy, report.clone(), "git", move || -> Store {
                    let running = read::running_pids();
                    let worktrees = read::read_worktrees(&c, running.as_ref());
                    let notes = match &worktrees {
                        Reading::Found(list) => Reading::Found(read::all_notes(list)),
                        // No worktree list (no git): the checkout's own notes still show.
                        _ => Reading::Found(read::read_notes(&c, &tr("main_checkout"))),
                    };
                    Box::new(move |r: &mut Report| {
                        r.worktrees = worktrees;
                        r.notes = notes;
                        r.git_at = Some(read::now_unix());
                    })
                });
            }
            if let Some(busy) = p.delivery.due(DELIVERY_EVERY) {
                let c = checkout.clone();
                spawn(ctx, busy, report.clone(), "delivery", move || -> Store {
                    let d = match delivery::check(&c) {
                        Ok(d) => Reading::Found(d),
                        Err(e) => Reading::Unreadable(e),
                    };
                    Box::new(move |r: &mut Report| {
                        r.delivery = d;
                        r.delivery_at = Some(read::now_unix());
                    })
                });
            }
            if let Some(busy) = p.release.due(RELEASE_EVERY) {
                spawn(ctx, busy, report.clone(), "release", move || -> Store {
                    let rel = read::fetch_latest_release();
                    Box::new(move |r: &mut Report| {
                        r.release = rel;
                        r.release_at = Some(read::now_unix());
                    })
                });
            }
        }
    }
    let copy = p.report.lock().map(|r| r.clone()).unwrap_or_default();
    copy
}

/// The checkout's own files: the journal tail, the TIER 0 headings, Cargo.toml's version.
fn read_files(checkout: &std::path::Path) -> (Reading<Vec<read::Decision>>, Reading<Vec<String>>, Reading<String>) {
    let decisions = read::read_text(&checkout.join("data").join(JOURNAL)).then(|t| read::journal_tail(&t, DECISIONS));
    let tier0 = read::read_text(&checkout.join("docs").join("PRIORITIES.md")).then(|t| Ok(read::tier0_headings(&t, HEADINGS)));
    let version = read::read_text(&checkout.join("Cargo.toml"))
        .then(|t| read::cargo_version(&t).ok_or_else(|| "Cargo.toml names no version".to_string()));
    (decisions, tier0, version)
}

/// Show a fixed report and never refresh (the snapshot test).
#[cfg(test)]
pub(crate) fn pin(report: Report) {
    if let Ok(mut p) = panel().lock() {
        p.pinned = true;
        if let Ok(mut r) = p.report.lock() {
            *r = report;
        }
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────

pub fn draw(ctx: &egui::Context, theme: &Theme, _state: &mut GuiState) {
    let report = refresh(ctx);
    let now = read::now_unix();
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| draw_report(ui, theme, &report, now));
        });
    // The "ago" times tick, and a finished job's repaint may land while nothing else moves.
    ctx.request_repaint_after(Duration::from_secs(1));
}

/// The whole panel for one report. Pure: the tests and the snapshot draw fixtures through it.
pub(crate) fn draw_report(ui: &mut egui::Ui, theme: &Theme, r: &Report, now: i64) {
    ui.label(RichText::new(tr("title")).size(theme.font_size_title).color(theme.text_primary()));
    ui.label(RichText::new(tr("intro")).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_sm);
    match &r.checkout {
        Reading::Found(path) => {
            ui.label(
                RichText::new(trf("checkout", &[("path", path.display().to_string())]))
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        }
        Reading::Waiting => {
            muted(ui, theme, &tr("waiting"));
            return;
        }
        _ => {
            widgets::card(ui, theme, |ui| muted(ui, theme, &tr("no_checkout")));
            return;
        }
    }
    ui.add_space(theme.spacing_sm);

    let wide = ui.available_width() >= 980.0;
    if wide {
        ui.columns(2, |cols| {
            worktrees_card(&mut cols[0], theme, r, now);
            cols[0].add_space(theme.spacing_sm);
            notes_card(&mut cols[0], theme, r, now);
            journal_card(&mut cols[1], theme, r);
            cols[1].add_space(theme.spacing_sm);
            tier0_card(&mut cols[1], theme, r);
            cols[1].add_space(theme.spacing_sm);
            version_card(&mut cols[1], theme, r, now);
            cols[1].add_space(theme.spacing_sm);
            this_pc_card(&mut cols[1], theme, r, now);
        });
    } else {
        worktrees_card(ui, theme, r, now);
        ui.add_space(theme.spacing_sm);
        notes_card(ui, theme, r, now);
        for card in [journal_card as fn(&mut egui::Ui, &Theme, &Report), tier0_card] {
            ui.add_space(theme.spacing_sm);
            card(ui, theme, r);
        }
        ui.add_space(theme.spacing_sm);
        version_card(ui, theme, r, now);
        ui.add_space(theme.spacing_sm);
        this_pc_card(ui, theme, r, now);
    }
    ui.add_space(theme.spacing_sm);
    let when = |t: Option<i64>| t.map(|t| ago(now, t)).unwrap_or_else(|| tr("waiting"));
    muted(
        ui,
        theme,
        &trf(
            "read_times",
            &[("files", when(r.files_at)), ("git", when(r.git_at)), ("delivery", when(r.delivery_at)), ("github", when(r.release_at))],
        ),
    );
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_muted()));
}

/// A small painted status dot (no glyph: the font's circles are not reliable, icon_glyph_lint).
fn dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

/// "Label: text" with a status dot, wrapping.
fn status_row(ui: &mut egui::Ui, theme: &Theme, color: Color32, label: &str, text: &str) {
    ui.horizontal_wrapped(|ui| {
        dot(ui, color);
        ui.label(RichText::new(format!("{label}:")).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_primary()));
    });
}

/// One line, cut to the width with its whole text on hover.
fn one_line(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_small).color(color)).truncate()).on_hover_text(text);
}

/// What to show for a reading that has nothing to draw: waiting, absent or unreadable.
fn not_found_text<T>(reading: &Reading<T>) -> Option<String> {
    match reading {
        Reading::Found(_) => None,
        Reading::Waiting => Some(tr("waiting")),
        Reading::Absent(what) => Some(trf("absent", &[("what", what.clone())])),
        Reading::Unreadable(why) => Some(trf("unreadable", &[("why", why.clone())])),
    }
}

fn heading(ui: &mut egui::Ui, theme: &Theme, title: &str, hint: &str) {
    ui.label(RichText::new(title).size(theme.font_size_heading).color(theme.text_primary()));
    muted(ui, theme, hint);
    ui.add_space(theme.spacing_xs);
}

fn worktrees_card(ui: &mut egui::Ui, theme: &Theme, r: &Report, now: i64) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, theme, &tr("worktrees"), &tr("worktrees_hint"));
        let list = match &r.worktrees {
            Reading::Found(list) => list,
            Reading::Absent(_) => return muted(ui, theme, &tr("git_missing")),
            other => return muted(ui, theme, &not_found_text(other).unwrap_or_default()),
        };
        if list.is_empty() {
            return muted(ui, theme, &tr("no_worktrees"));
        }
        for (i, w) in list.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            worktree_row(ui, theme, w, now);
        }
    });
}

fn worktree_row(ui: &mut egui::Ui, theme: &Theme, w: &Worktree, now: i64) {
    let dirty = w.uncommitted.is_some_and(|n| n > 0);
    let running = w.holder_running == Some(true) || (w.locked.is_some() && w.holder_running.is_none());
    let color = if dirty {
        theme.warning()
    } else if running {
        theme.info()
    } else {
        theme.text_muted()
    };
    ui.horizontal_wrapped(|ui| {
        dot(ui, color);
        let name = if w.main { tr("main_checkout") } else { w.name() };
        ui.label(RichText::new(name).size(theme.font_size_body).color(theme.text_primary()).strong()).on_hover_text(&w.path);
        // An agent's branch is "worktree-<folder>": say it only when it says something more.
        let branch = match &w.branch {
            Some(b) if *b == format!("worktree-{}", w.name()) => None,
            Some(b) => Some(b.clone()),
            None => Some(tr("detached")),
        };
        if let Some(b) = branch {
            ui.label(RichText::new(b).size(theme.font_size_small).color(theme.text_secondary()));
        }
        let (changes, changes_color) = match w.uncommitted {
            Some(0) => (tr("clean"), theme.text_muted()),
            Some(n) => (trf("uncommitted", &[("n", n.to_string())]), theme.warning()),
            None if w.prunable => (tr("prunable"), theme.danger()),
            None => (tr("uncommitted_unknown"), theme.text_muted()),
        };
        ui.label(RichText::new(changes).size(theme.font_size_small).color(changes_color));
        let held = match (&w.locked, w.holder_running) {
            (None, _) => None,
            (Some(_), Some(true)) => Some((tr("held_running"), theme.info())),
            (Some(_), Some(false)) => Some((tr("held_gone"), theme.text_muted())),
            (Some(_), None) => Some((tr("held"), theme.info())),
        };
        if let Some((text, c)) = held {
            ui.label(RichText::new(text).size(theme.font_size_small).color(c));
        } else if !w.main {
            ui.label(RichText::new(tr("not_held")).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    let last = match (w.committed_at, &w.subject) {
        (Some(t), Some(s)) => format!("{}: {s}", ago(now, t)),
        _ => tr("no_commit"),
    };
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        one_line(ui, theme, &last, theme.text_secondary());
    });
}

fn notes_card(ui: &mut egui::Ui, theme: &Theme, r: &Report, now: i64) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, theme, &tr("notes"), &tr("notes_hint"));
        match &r.notes {
            Reading::Found(list) if list.is_empty() => muted(ui, theme, &tr("no_notes")),
            Reading::Found(list) => {
                for n in list.iter().take(NOTES) {
                    let when = n.modified.map(|t| ago(now, t)).unwrap_or_else(|| "?".into());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(trf("note_line", &[("place", n.place.clone()), ("ago", when)]))
                                .size(theme.font_size_small)
                                .color(theme.accent()),
                        )
                        .on_hover_text(&n.path);
                        one_line(ui, theme, &n.title, theme.text_primary());
                    });
                }
            }
            other => muted(ui, theme, &not_found_text(other).unwrap_or_default()),
        }
    });
}

fn journal_card(ui: &mut egui::Ui, theme: &Theme, r: &Report) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, theme, &tr("journal"), &tr("journal_hint"));
        match &r.decisions {
            Reading::Found(list) => {
                for d in list {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&d.date).size(theme.font_size_small).color(theme.accent()));
                        one_line(ui, theme, &d.text, theme.text_primary());
                    });
                }
            }
            other => muted(ui, theme, &not_found_text(other).unwrap_or_default()),
        }
    });
}

fn tier0_card(ui: &mut egui::Ui, theme: &Theme, r: &Report) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, theme, &tr("tier0"), &tr("tier0_hint"));
        match &r.tier0 {
            Reading::Found(list) => {
                for (i, h) in list.iter().enumerate() {
                    one_line(ui, theme, &format!("{}. {h}", i + 1), theme.text_primary());
                }
            }
            other => muted(ui, theme, &not_found_text(other).unwrap_or_default()),
        }
    });
}

fn version_card(ui: &mut egui::Ui, theme: &Theme, r: &Report, now: i64) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, theme, &tr("version"), &tr("delivery_hint"));
        let local = r.version.found().cloned();
        match &r.version {
            Reading::Found(v) => status_row(ui, theme, theme.text_muted(), &tr("this_checkout"), &format!("v{v}")),
            other => status_row(ui, theme, theme.warning(), &tr("this_checkout"), &not_found_text(other).unwrap_or_default()),
        }
        match &r.release {
            Reading::Found(rel) => {
                let (verdict, color) = match local.as_deref().map(|l| read::compare_versions(l, &rel.tag)) {
                    Some(std::cmp::Ordering::Equal) => (tr("in_sync"), theme.success()),
                    Some(std::cmp::Ordering::Greater) => (tr("ahead"), theme.warning()),
                    Some(std::cmp::Ordering::Less) => (tr("behind"), theme.danger()),
                    None => (String::new(), theme.text_muted()),
                };
                let when = rel.published_at.map(|t| ago(now, t)).unwrap_or_else(|| "?".into());
                let mut text = trf("published", &[("tag", rel.tag.clone()), ("ago", when)]);
                if !verdict.is_empty() {
                    text = format!("{text}; {verdict}");
                }
                status_row(ui, theme, color, &tr("latest_release"), &text);
            }
            other => status_row(ui, theme, theme.text_muted(), &tr("latest_release"), &not_found_text(other).unwrap_or_default()),
        }
        let (text, color) = delivery_text(&r.delivery, local.as_deref(), theme);
        status_row(ui, theme, color, &tr("taskbar"), &text);
    });
}

/// The taskbar exe's line, in check-delivery's terms.
pub(crate) fn delivery_text(d: &Reading<delivery::Delivery>, local: Option<&str>, theme: &Theme) -> (String, Color32) {
    use delivery::Problem;
    let d = match d {
        Reading::Found(d) => d,
        other => return (not_found_text(other).unwrap_or_default(), theme.text_muted()),
    };
    let Some(first) = d.problems.first() else {
        let v = d.taskbar_version.clone().or(local.map(str::to_string)).unwrap_or_else(|| "?".into());
        let key = if d.taskbar_version.is_some() && d.taskbar_version.as_deref() != local { "delivered_trailing" } else { "delivered" };
        return (trf(key, &[("v", v)]), theme.success());
    };
    let text = match first {
        Problem::TaskbarMissing => tr("taskbar_missing"),
        Problem::TaskbarNoStamp => tr("taskbar_no_stamp"),
        Problem::NotBuilt { differ, first } => trf("not_built", &[("n", differ.to_string()), ("first", first.clone())]),
        Problem::NotDelivered { differ, first } => trf("not_delivered", &[("n", differ.to_string()), ("first", first.clone())]),
        Problem::BuiltMissing => tr("built_missing"),
        Problem::BuiltStale => tr("built_stale"),
    };
    let rest: Vec<String> = d.problems[1..]
        .iter()
        .map(|p| match p {
            Problem::BuiltMissing => tr("built_missing"),
            Problem::BuiltStale => tr("built_stale"),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect();
    let text = if rest.is_empty() { text } else { format!("{text}; {}", rest.join("; ")) };
    (text, theme.danger())
}

fn this_pc_card(ui: &mut egui::Ui, theme: &Theme, r: &Report, now: i64) {
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tr("this_pc")).size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        let (text, color) = share_guard_text(&r.share_guard, now, theme);
        status_row(ui, theme, color, &tr("share_guard"), &text);
        let (text, color) = disk_guard_text(&r.disk_guard, now, theme);
        status_row(ui, theme, color, &tr("disk_guard"), &text);
        let (text, color) = boot_lock_text(&r.boot_lock, now, theme);
        status_row(ui, theme, color, &tr("boot_lock"), &text);
        let (text, color) = build_slot_text(&r.build_slot, theme);
        status_row(ui, theme, color, &tr("build_slot"), &text);
        if let Reading::Found(list) = &r.build_slot {
            for e in list.iter().take(SLOT_LINES) {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    one_line(ui, theme, &slot_line(e, now), if e.waiting { theme.text_muted() } else { theme.text_secondary() });
                });
            }
        }
    });
}

pub(crate) fn share_guard_text(g: &Reading<read::ShareGuard>, now: i64, theme: &Theme) -> (String, Color32) {
    match g {
        Reading::Found(g) if now - g.at <= read::SHARE_GUARD_FRESH_SECS => (
            trf(
                "share_running",
                &[
                    ("ago", ago(now, g.at)),
                    ("n", g.dev_processes_now.to_string()),
                    ("kept", g.kept.to_string()),
                    ("logical", g.logical.to_string()),
                ],
            ),
            theme.success(),
        ),
        Reading::Found(g) => (trf("share_stopped", &[("ago", ago(now, g.at))]), theme.warning()),
        Reading::Absent(_) => (tr("share_never"), theme.warning()),
        other => (not_found_text(other).unwrap_or_default(), theme.text_muted()),
    }
}

pub(crate) fn disk_guard_text(g: &Reading<read::DiskGuard>, now: i64, theme: &Theme) -> (String, Color32) {
    match g {
        Reading::Found(g) => {
            let what = match (&g.skipped, g.cleared) {
                (Some(why), _) => trf("disk_skipped", &[("why", why.clone())]),
                (None, 0) => tr("disk_nothing"),
                (None, n) => trf("disk_cleared", &[("n", n.to_string())]),
            };
            let late = now - g.at > read::DISK_GUARD_LATE_SECS;
            let text = trf(
                if late { "disk_late" } else { "disk_ran" },
                &[
                    ("ago", ago(now, g.at)),
                    ("what", what),
                    ("pct", format!("{:.0}", g.used_pct)),
                    ("free", format!("{:.0}", g.free_gb)),
                ],
            );
            (text, if late { theme.warning() } else { theme.success() })
        }
        Reading::Absent(_) => (tr("disk_never"), theme.warning()),
        other => (not_found_text(other).unwrap_or_default(), theme.text_muted()),
    }
}

/// One compile in the build slot, as one line: what, where, how much memory, since when.
pub(crate) fn slot_line(e: &read::SlotEntry, now: i64) -> String {
    let vars = [
        ("crate", e.crate_name.clone()),
        ("kind", e.kind.clone()),
        ("place", e.place()),
        ("gb", format!("{:.1}", e.mb as f64 / 1024.0)),
        ("ago", e.since.map(|t| ago(now, t)).unwrap_or_else(|| "?".into())),
    ];
    trf(if e.waiting { "slot_waiting" } else { "slot_compiling" }, &vars)
}

/// The build slot's line: not set up, nothing compiling, or how many compile and wait.
pub(crate) fn build_slot_text(s: &Reading<Vec<read::SlotEntry>>, theme: &Theme) -> (String, Color32) {
    match s {
        Reading::Found(list) if list.is_empty() => (tr("slot_idle"), theme.success()),
        Reading::Found(list) => {
            let waiting = list.iter().filter(|e| e.waiting).count();
            let compiling = list.len() - waiting;
            let text = trf("slot_summary", &[("compiling", compiling.to_string()), ("waiting", waiting.to_string())]);
            (text, if waiting > 0 { theme.warning() } else { theme.info() })
        }
        Reading::Absent(_) => (tr("slot_none"), theme.text_muted()),
        other => (not_found_text(other).unwrap_or_default(), theme.text_muted()),
    }
}

pub(crate) fn boot_lock_text(l: &Reading<LockState>, now: i64, theme: &Theme) -> (String, Color32) {
    let since = |t: Option<i64>| t.map(|t| ago(now, t)).unwrap_or_else(|| "?".into());
    match l {
        Reading::Found(LockState::Free) => (tr("lock_free"), theme.success()),
        Reading::Found(LockState::Held(h)) => (
            trf(
                "lock_held",
                &[("rig", h.rig.clone()), ("pid", h.pid.map(|p| p.to_string()).unwrap_or_default()), ("ago", since(h.since))],
            ),
            theme.info(),
        ),
        Reading::Found(LockState::Stale(h)) => (
            trf("lock_stale", &[("rig", h.rig.clone()), ("pid", h.pid.map(|p| p.to_string()).unwrap_or_default())]),
            theme.text_muted(),
        ),
        Reading::Found(LockState::Unreadable) => (tr("lock_unreadable"), theme.warning()),
        other => (not_found_text(other).unwrap_or_default(), theme.text_muted()),
    }
}

#[cfg(test)]
pub(crate) mod tests;
