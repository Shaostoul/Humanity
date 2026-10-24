//! The How-to page (docs/design/how-to.md, increment 1, 2026-10-23): step-by-step procedures for
//! real life and for this app, from data/how_to/ (crate::how_to is the model, the checker and the
//! runs; this file and how_to/ draw them).
//!
//! THREE PANES (widgets::panes, as the Files page): the rail (search, Doing now, the category
//! tree with counts), the list of how-tos in the chosen branch, and the how-to itself, in the order
//! that is its safety design: what it is and how dangerous, Short or Full, Stop and call someone
//! if, What you need, the steps, It worked if, If something goes wrong, Learn more, Your history,
//! Sources. The split and the Short/Full choice are kept in the settings (config.json
//! `how_to_layout`, `how_to_view`), and "How this page works" is folded by the heading
//! (`how_to_how_open`).
//!
//! RUNS. Start (or ticking any step) begins a run; every tick is written to disk at once
//! (crate::how_to::runs), so leaving the page, closing the app or restarting loses nothing, and a
//! timer keeps running while the app is closed. Only the person ticks a real step.
//!
//! LOADING. The how-tos are read and checked on a worker thread the first time the page opens
//! (a few milliseconds for the six shipped; the budget is in the design), never on the frame. The
//! page asks for a repaint once a second only while a timer is on screen.
//!
//! Web mirror: web/pages/how-to.html + how-to-app.js, the same data, the same words
//! (data/how_to/words.json) and the same rules (web/shared/how-to-logic.js).

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::how_to::library::{Library, Pages};
use crate::how_to::runs::{Run, RunStore};
use crate::how_to::view::View;

mod list;
mod rail;
mod steps;
mod view;
pub mod words;

#[cfg(test)]
mod tests;

pub use words::{w, wf};

/// The default split of the three panes: rail, list, how-to.
pub const SHARES: [f32; 3] = [0.22, 0.30, 0.48];
/// The narrowest a pane may be dragged.
const MIN_PANE: f32 = 220.0;

/// How the list is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// The tree's own order: categories as the data orders them, then titles.
    #[default]
    Suggested,
    AToZ,
    Quickest,
}

/// The page's state (GuiState::how_to).
pub struct HowToState {
    /// Everything loaded, once loaded.
    pub lib: Option<Arc<Library>>,
    loading: Option<Receiver<(Library, Vec<Run>)>>,
    /// This person's runs, newest first.
    pub runs: Vec<Run>,
    pub store: RunStore,
    /// The chosen category, or None for every how-to.
    pub branch: Option<String>,
    /// The open how-to's id (written or planned).
    pub open: Option<String>,
    pub search: String,
    pub sort: Sort,
    pub easy_only: bool,
    pub quick_only: bool,
    /// Branches unfolded in the rail.
    pub expanded: HashSet<String>,
    /// What is being typed into a measure step, by step id.
    pub typing: HashMap<String, String>,
    /// The last thing that went wrong saving a run, shown until the next save works.
    pub save_error: Option<String>,
    /// Timers already announced, so a notice is given once (run id, step id).
    announced: HashSet<(String, String)>,
    /// A step to scroll to on the next frame.
    pub scroll_to: Option<String>,
}

impl Default for HowToState {
    fn default() -> Self {
        HowToState {
            lib: None,
            loading: None,
            runs: Vec::new(),
            store: RunStore::new(crate::how_to::runs::default_dir()),
            branch: None,
            open: None,
            search: String::new(),
            sort: Sort::default(),
            easy_only: false,
            quick_only: false,
            expanded: HashSet::new(),
            typing: HashMap::new(),
            save_error: None,
            announced: HashSet::new(),
            scroll_to: None,
        }
    }
}

impl HowToState {
    /// Load now, on this thread (the snapshots and the tests).
    pub fn load_now(&mut self, data_dir: &std::path::Path) {
        self.lib = Some(Arc::new(Library::load(data_dir, Some(app_pages()))));
        self.runs = self.store.list();
    }

    /// The open run of a how-to, if there is one.
    pub fn open_run(&self, id: &str) -> Option<&Run> {
        self.runs.iter().find(|r| r.how_to == id && r.is_open())
    }

    /// Write a run and keep it in the list (newest first).
    pub fn save_run(&mut self, run: Run) {
        match self.store.save(&run) {
            Ok(()) => self.save_error = None,
            Err(e) => self.save_error = Some(e),
        }
        match self.runs.iter_mut().find(|r| r.run == run.run) {
            Some(slot) => *slot = run,
            None => self.runs.insert(0, run),
        }
    }

    /// Open a how-to, unfolding its branch in the rail.
    pub fn open(&mut self, id: &str) {
        self.open = Some(id.to_string());
        if let Some(lib) = &self.lib {
            let cat = lib.find(id).map(|l| l.how_to.category.clone()).or_else(|| lib.find_planned(id).map(|p| p.category.clone()));
            if let Some(cat) = cat {
                for c in lib.path_to(&cat) {
                    self.expanded.insert(c);
                }
            }
        }
    }
}

/// The page ids a go step may name, each with the sections it can open at (Settings' sections).
pub fn app_pages() -> Pages {
    let mut out = Pages::new();
    for &p in crate::gui::dispatch::ALL_PAGES {
        out.insert(crate::gui::dispatch::page_id(p).to_string(), Vec::new());
    }
    out.insert("settings".to_string(), crate::gui::pages::settings::SECTIONS.iter().map(|(_, c)| section_id(*c)).collect());
    out
}

/// A Settings section's id in a go step: its name, lower case.
pub fn section_id(c: crate::gui::SettingsCategory) -> String {
    format!("{c:?}").to_lowercase()
}

/// Open the app at a go step's page and section.
pub fn go(state: &mut GuiState, page: &str, section: Option<&str>) {
    let Some(target) = crate::gui::dispatch::page_from_id(page) else { return };
    if target == crate::gui::GuiPage::Settings {
        if let Some(cat) = section.and_then(|s| crate::gui::pages::settings::SECTIONS.iter().find(|(_, c)| section_id(*c) == s)) {
            state.settings.category = cat.1;
            state.settings.scroll_to_section = Some(cat.1);
        }
    }
    state.push_nav_to(target);
}

fn poll_loading(state: &mut GuiState) {
    let h = &mut state.how_to;
    if h.lib.is_some() {
        return;
    }
    if h.loading.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        let data_dir = crate::data_dir();
        let store = h.store.clone();
        let pages = app_pages();
        std::thread::Builder::new()
            .name("how-to-load".into())
            .spawn(move || {
                let lib = Library::load(&data_dir, Some(pages));
                let _ = tx.send((lib, store.list()));
            })
            .ok();
        h.loading = Some(rx);
    }
    if let Some(rx) = &h.loading {
        if let Ok((lib, runs)) = rx.try_recv() {
            h.lib = Some(Arc::new(lib));
            h.runs = runs;
            h.loading = None;
        }
    }
}

/// Draw the page.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    poll_loading(state);
    egui::CentralPanel::default().frame(widgets::page_frame(theme)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            widgets::section_header(ui, theme, w("title"));
            ui.add_space(theme.spacing_sm);
            if widgets::how_it_works::toggle(ui, theme, state.settings.how_to_how_open) {
                state.settings.how_to_how_open = !state.settings.how_to_how_open;
                state.settings_dirty = true;
            }
        });
        ui.label(RichText::new(w("line")).color(theme.text_muted()).size(theme.font_size_small));
        if state.settings.how_to_how_open {
            widgets::how_it_works::panel(ui, theme, |ui| {
                for key in ["how_lists", "how_views", "how_runs", "how_review", "how_ai"] {
                    widgets::how_it_works::line(ui, theme, w(key));
                }
                let folder = state.how_to.store.folder().display().to_string();
                widgets::how_it_works::line(ui, theme, &wf("runs_kept", &[("folder", &folder)]));
            });
        }
        if let Some(e) = &state.how_to.save_error {
            ui.label(RichText::new(wf("save_failed", &[("why", e)])).color(theme.danger()).size(theme.font_size_small));
        }
        ui.add_space(theme.spacing_sm);
        let Some(lib) = state.how_to.lib.clone() else {
            ui.label(RichText::new(w("loading")).color(theme.text_muted()));
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        announce_timers(ui, &lib, state);
        let arrange = widgets::panes::arrange_for(ui.available_size(), 3, MIN_PANE, theme.spacing_md);
        let mut shares = widgets::panes::checked(&state.settings.how_to_layout, &SHARES);
        let moved = widgets::panes::show(ui, theme, egui::Id::new("how_to_panes"), arrange, &mut shares, &SHARES, MIN_PANE, |ui, i, _card| {
            let id = egui::Id::new(("how_to_pane", i));
            egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| match i {
                0 => rail::draw(ui, theme, state, &lib),
                1 => list::draw(ui, theme, state, &lib),
                _ => view::draw(ui, theme, state, &lib),
            });
        });
        if moved.changed {
            state.settings.how_to_layout = shares;
        }
        if moved.settled {
            state.settings_dirty = true;
        }
    });
}

/// Post a notice once when a running timer ends while the app is open, and ask for a repaint each
/// second while any timer runs (only then: an idle page costs nothing per frame).
fn announce_timers(ui: &egui::Ui, lib: &Library, state: &mut GuiState) {
    let now = crate::persistence::now_ms();
    let mut due = Vec::new();
    let mut running = false;
    for r in state.how_to.runs.iter().filter(|r| r.is_open()) {
        for step in r.waits.keys() {
            match r.wait_left_ms(step, now) {
                Some(0) => {
                    if !r.is_done(step) && !state.how_to.announced.contains(&(r.run.clone(), step.clone())) {
                        due.push((r.run.clone(), step.clone(), r.how_to.clone()));
                    }
                }
                Some(_) => running = true,
                None => {}
            }
        }
    }
    let t = ui.input(|i| i.time);
    for (run, step, how_to) in due {
        let title = lib.find(&how_to).map(|l| l.how_to.title.clone()).unwrap_or(how_to);
        state.notice(wf("wait_ready", &[("title", &title)]), t);
        state.how_to.announced.insert((run, step));
    }
    if running {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }
}

/// The view the person chose.
pub fn chosen_view(state: &GuiState) -> View {
    state.settings.how_to_view
}

/// The words of a hazard.
pub fn hazard_word(h: crate::how_to::model::Hazard) -> &'static str {
    use crate::how_to::model::Hazard;
    match h {
        Hazard::None => w("hazard_none"),
        Hazard::Caution => w("hazard_caution"),
        Hazard::Serious => w("hazard_serious"),
        Hazard::Lethal => w("hazard_lethal"),
    }
}

/// The colour of a hazard's word and icon. Never alone: the word is always beside it.
pub fn hazard_colour(theme: &Theme, h: crate::how_to::model::Hazard) -> egui::Color32 {
    use crate::how_to::model::Hazard;
    match h {
        Hazard::None => theme.text_muted(),
        Hazard::Caution => theme.info(),
        Hazard::Serious => theme.warning(),
        Hazard::Lethal => theme.danger(),
    }
}

/// A hazard's icon and word, in a row.
pub fn hazard_badge(ui: &mut egui::Ui, theme: &Theme, h: crate::how_to::model::Hazard) {
    if h == crate::how_to::model::Hazard::None {
        return;
    }
    let colour = hazard_colour(theme, h);
    let size = theme.font_size_small + 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if h == crate::how_to::model::Hazard::Lethal {
        widgets::icons::paint_warning_filled(ui.painter(), rect, colour, theme.bg_card());
    } else {
        widgets::icons::paint_warning(ui.painter(), rect, colour);
    }
    ui.label(RichText::new(hazard_word(h)).color(colour).size(theme.font_size_small).strong());
}

/// The difficulty in words.
pub fn difficulty_word(d: u8) -> &'static str {
    match d {
        1 => w("difficulty_1"),
        2 => w("difficulty_2"),
        3 => w("difficulty_3"),
        4 => w("difficulty_4"),
        _ => w("difficulty_5"),
    }
}

/// The review state in words.
pub fn review_words(l: &crate::how_to::library::Loaded) -> String {
    use crate::how_to::model::Status;
    if !l.review_current {
        return w("review_changed").to_string();
    }
    let r = &l.how_to.review;
    match r.status {
        Status::Verified => wf("review_verified", &[("date", r.checked.as_deref().unwrap_or(""))]),
        Status::Sourced => w("review_sourced").to_string(),
        Status::Stub | Status::Absent => w("review_stub").to_string(),
    }
}

/// A how-to's time in words: "10 min", "15 min plus 12 h 0 min waiting".
pub fn time_words(t: &crate::how_to::model::Time) -> String {
    let active = words::minutes(t.active_min as u64);
    if t.waiting_min == 0 {
        active
    } else {
        format!("{active} {}", wf("plus_waiting", &[("time", &words::minutes(t.waiting_min as u64))]))
    }
}

/// An item's name, from items.csv, or its id as words.
pub fn item_name(lib: &Library, id: &str) -> String {
    lib.item_names.get(id).cloned().unwrap_or_else(|| crate::how_to::view::item_words(id))
}

/// Whether a how-to or a branch shows: developer ones only with Developer mode on.
pub fn audience_shows(theme: &Theme, a: crate::how_to::model::Audience) -> bool {
    a != crate::how_to::model::Audience::Developer || theme.nav_dev_visible
}
