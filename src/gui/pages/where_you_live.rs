//! "Choose where you live" (ship homes increment 6, lane 1E, 2026-10-12;
//! docs/design/ship-homes-increment-6-plan.md section 3.9): the one widget that says where you live
//! and lets you choose, drawn in four places.
//!
//! - Settings > Gameplay, "Where you live" (`draw_settings_section`): "In this world you live in a
//!   Homestead (a family of three).", the household sub-choice for the live save, and where to go
//!   to live somewhere else. It replaced "Home Design" (Family or Solo, one setting per install),
//!   which stopped doing anything when Wave 0 made the household file follow the save.
//! - The Home page (`draw_home_line`): the same line, read only.
//! - Characters (Play), for lane 2B: the new-home row's choice (`draw_choice`, with `draft_choice`
//!   for its Enter bar) and a server's card (`draw_server`), drawn from the server's `game_home`
//!   answer.
//!
//! WORDS. Every start's name and sentence, and why a start is not offered yet, comes from
//! data/homes/starts.ron (`Starts`) or, on a server's card, from that server's own answer
//! (`HomeAnswer`), never from a copy here. The shared places are the zone types' own labels
//! (data/blueprints/zone_types.ron). The words this file owns are about the choice itself: its
//! title and hint, what each way of looking after food, water and power means (`engagement_line`),
//! and the server card's sentences. None of them, and no start's words (a test holds both), says
//! tier, rank, level or berth: where you live is how much space you hold and how much looking
//! after it needs, never a step on a ladder (docs/design/engagement-modes.md).
//!
//! THE HOUSEHOLD IS THE SAVE'S. Choosing "Just you" in Settings changes the live save's household
//! (`save_load::ActiveHome::household`), which the save writes at its next save and the household
//! file the world loads comes from (`machines::home_ron_path`). It never touches AppConfig: since
//! increment 6 each save keeps its own, and a test holds that too.
//!
//! The widget's own choices (the start picked on the new-home row, the household picked per start)
//! live in a `thread_local!`, the way homes.rs keeps its scale. The website's mirror of the
//! server's card is web/chat/chat-home.js. Colours are theme tokens only.

use std::cell::RefCell;
use std::collections::BTreeMap;

use egui::{Color32, RichText};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button};
use crate::gui::{GuiState, HintDisplay};
use crate::ship::starts::{Engagement, EngagementDefaults, HomeAnswer, Start, Starts};

// ── The words this file owns ─────────────────────────────────────────────────────────────────────

/// The choice's title.
pub const TITLE: &str = "Choose where you live";
/// Under the title (muted): what a start changes, and what it never changes.
pub const HINT: &str = "Where you live sets how much space you hold and how much looking after it needs. \
     It never changes how deep the world runs or how hard it is: that is your play mode.";
/// The heading in Settings > Gameplay and on the Home page.
pub const HEADING: &str = "Where you live";
/// After the default start's name on its row.
pub const DEFAULT_MARK: &str = " (most people start here)";
/// Under the household sub-choice in Settings.
pub const TAKES_EFFECT: &str = "Takes effect the next time you enter the world.";
/// Settings: how to live somewhere else.
/// Since increment 6 your own home is one save on this computer, and Characters offers the choice
/// of a new home only while it has none (showroom.rs `NEW_HOME_TAKEN`); this sentence is shown in
/// the world, where that save exists, so it never offers that.
pub const ELSEWHERE: &str = "Your own home on this computer stays the one you have. To live somewhere else, choose a home \
     on a server's card in Characters (Play). Moving house comes in a later update.";
/// A server's card when the person holds no home on that server's ship.
pub const NO_HOME_HERE: &str = "You have no home on this server yet.";
/// A server's card, always, last (muted).
pub const SERVER_NOTE: &str =
    "On a server you start fresh: only your name and look come with you. Your home there is kept on this computer.";
/// A server's card while its answer is on its way.
pub const ASKING: &str = "Asking the server where you live...";
/// A server's card with no answer and nothing on its way (a server from before increment 6).
pub const NO_ANSWER: &str = "This server has not answered. It may not offer homes yet.";
/// A server's card when the server offers no start at all.
pub const NONE_OFFERED: &str = "This server has no homes to choose from right now.";
/// A refusal the server gave no sentence for.
pub const REFUSED_NO_WORDS: &str = "The server did not give you that home.";
/// A choice the server made.
pub const CHOSEN: &str = "Chosen: this is your home on this server now.";
/// The button on a server's card.
pub const CHOOSE: &str = "Choose";

/// What food is, for each way of looking after it.
fn food_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => "you grow it.",
        Engagement::Trade => "you buy it.",
        Engagement::Automation => "your machines grow it.",
        Engagement::Cooperative => "shared dinners with your block.",
        Engagement::Background => "free crew meals in the mess hall.",
    }
}

/// What water and power are, when both are looked after the same way.
fn water_and_power_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => "you collect and make them.",
        Engagement::Trade => "you buy them.",
        Engagement::Automation => "your machines collect and make them.",
        Engagement::Cooperative => "shared with your block.",
        Engagement::Background => "the ship's, counted on your meter.",
    }
}

/// What water is, when power is looked after another way.
fn water_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => "you collect it.",
        Engagement::Trade => "you buy it.",
        Engagement::Automation => "your machines collect it.",
        Engagement::Cooperative => "shared with your block.",
        Engagement::Background => "the ship's, counted on your meter.",
    }
}

/// What power is, when water is looked after another way.
fn power_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => "you make it.",
        Engagement::Trade => "you buy it.",
        Engagement::Automation => "your machines make it.",
        Engagement::Cooperative => "shared with your block.",
        Engagement::Background => "the ship's, counted on your meter.",
    }
}

// ── Words from data ──────────────────────────────────────────────────────────────────────────────

/// The starts file, read once per run: the widget is drawn every frame the page is open.
pub fn starts() -> &'static Starts {
    static STARTS: std::sync::OnceLock<Starts> = std::sync::OnceLock::new();
    STARTS.get_or_init(|| Starts::load(&crate::data_dir()))
}

/// "a Cabin", "an Apartment": a name as written, with its article.
pub fn a_or_an(name: &str) -> String {
    let article = if name.trim_start().starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) { "an" } else { "a" };
    format!("{article} {name}")
}

/// "Just you" -> "just you": a name from the data as it reads mid-sentence.
fn mid_sentence(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(first) => first.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// A start's name on its row: "Homestead (most people start here)" for the default start.
pub fn row_name(starts: &Starts, id: &str, name: &str) -> String {
    if id == starts.default {
        format!("{name}{DEFAULT_MARK}")
    } else {
        name.to_string()
    }
}

/// "Shared: the mess hall, the bathroom, the recreation room": the places aboard a start relies
/// on, each by its zone type's label (an id the zone types do not list reads as its own words).
/// None for a start that shares nothing.
pub fn shared_line(shared: &[String]) -> Option<String> {
    if shared.is_empty() {
        return None;
    }
    let names: Vec<String> = shared
        .iter()
        .map(|id| {
            let label = crate::ship::structure::zone_type(id).map(|z| z.label.clone()).unwrap_or_else(|| id.replace(['_', '-'], " "));
            format!("the {}", mid_sentence(&label))
        })
        .collect();
    Some(format!("Shared: {}", names.join(", ")))
}

/// One plain line from a start's engagement: "Food: free crew meals in the mess hall. Water and
/// power: the ship's, counted on your meter." Water and power are said together when they are
/// looked after the same way.
pub fn engagement_line(e: &EngagementDefaults) -> String {
    let food = format!("Food: {}", food_words(e.food));
    if e.water == e.energy {
        format!("{food} Water and power: {}", water_and_power_words(e.water))
    } else {
        format!("{food} Water: {} Power: {}", water_words(e.water), power_words(e.energy))
    }
}

/// The start a save names, or the default when the file does not list it (the household file
/// falls back the same way, `Starts::household`).
fn start_of<'a>(starts: &'a Starts, id: &str) -> &'a Start {
    starts.get(id).unwrap_or_else(|| starts.default_start())
}

/// "In this world you live in a Homestead (a family of three).": the live save's start and
/// household, in the start file's words.
pub fn live_in_line(starts: &Starts, start: &str, household: &str) -> String {
    let s = start_of(starts, start);
    let h = starts.household(&s.id, household);
    format!("In this world you live in {} ({}).", a_or_an(&s.name), mid_sentence(&h.name))
}

/// "Where you live here: a Cabin, c2.": the home a server's answer says the person holds, named
/// by the answer's own words for its start (else this game's starts file, else the plot's kind).
/// None when the answer says they hold none.
pub fn held_line(starts: &Starts, answer: &HomeAnswer) -> Option<String> {
    let held = answer.plot.as_ref()?;
    let name = answer
        .starts
        .iter()
        .find(|o| o.id == held.start)
        .map(|o| o.name.clone())
        .or_else(|| starts.get(&held.start).map(|s| s.name.clone()))
        .unwrap_or_else(|| held.kind.replace(['_', '-'], " "));
    Some(format!("Where you live here: {}, {}.", a_or_an(&name), held.id))
}

/// "12 free", "1 free", "none free".
pub fn free_text(n: u32) -> String {
    if n > 0 {
        format!("{n} free")
    } else {
        "none free".to_string()
    }
}

// ── The widget's own choices ─────────────────────────────────────────────────────────────────────

/// A choice the widget made: a start, and one of its households.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub start: String,
    pub household: String,
}

/// The start picked on the new-home row (None: the default) and the household picked per start.
#[derive(Default)]
struct Draft {
    start: Option<String>,
    households: BTreeMap<String, String>,
}

thread_local! {
    static DRAFT: RefCell<Draft> = RefCell::new(Draft::default());
}

/// The new-home row's choice as it stands: the start picked (an unknown or unoffered one is the
/// default, `Starts::resolve`) and the household picked for it (else its first).
pub fn draft_choice(starts: &Starts) -> Choice {
    DRAFT.with(|d| {
        let d = d.borrow();
        let start = starts.resolve(d.start.as_deref());
        let household = d.households.get(&start.id).map(String::as_str).unwrap_or("");
        Choice { start: start.id.clone(), household: starts.household(&start.id, household).id.clone() }
    })
}

/// Forget what was picked (the new-home row starts again at the default).
pub fn reset_draft() {
    DRAFT.with(|d| *d.borrow_mut() = Draft::default());
}

/// The household picked for `start` on a server's card (else its first); engine/save_slot.rs
/// `choose_on_server` takes it for the save it makes there.
pub fn draft_household(starts: &Starts, start: &str) -> String {
    let picked = DRAFT.with(|d| d.borrow().households.get(start).cloned()).unwrap_or_default();
    starts.household(start, &picked).id.clone()
}

/// Make `household` the live save's household (Settings' sub-choice). The save writes it at its
/// next save; the world loads its household file the next time it is entered. Never AppConfig.
pub fn set_household(household: &str) {
    let mut active = crate::save_load::active_home();
    active.household = household.to_string();
    crate::save_load::set_active_home(active);
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────────

fn say(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(color));
}

/// The title and its hint.
fn heading(ui: &mut egui::Ui, theme: &Theme) {
    ui.label(RichText::new(TITLE).size(theme.font_size_body).strong().color(theme.text_primary()));
    say(ui, theme, HINT, theme.text_muted());
}

/// A start's lines under its name: its sentence, what it shares aboard, and how its food, water
/// and power are looked after. `sentence` is the server's on a server's card; `local` is this
/// game's row of the same start, for the lines a server's answer does not carry. Greyed when the
/// start cannot be chosen.
fn row_lines(ui: &mut egui::Ui, theme: &Theme, sentence: &str, local: Option<&Start>, greyed: bool) {
    let (main, rest) = if greyed { (theme.text_muted(), theme.text_muted()) } else { (theme.text_secondary(), theme.text_muted()) };
    if !sentence.is_empty() {
        say(ui, theme, sentence, main);
    }
    if let Some(s) = local {
        if let Some(line) = shared_line(&s.shared) {
            say(ui, theme, &line, rest);
        }
        say(ui, theme, &engagement_line(&s.engagement), rest);
    }
}

/// The household sub-choice for `start` ("A family of three" / "Just you"), when it has two or
/// more; `current` is the one shown chosen. Returns the household clicked.
fn household_radios(ui: &mut egui::Ui, theme: &Theme, start: &Start, current: &str) -> Option<String> {
    if start.households.len() < 2 {
        return None;
    }
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        for h in &start.households {
            let selected = h.id == current;
            if ui.radio(selected, RichText::new(&h.name).size(theme.font_size_small).color(theme.text_primary())).clicked() && !selected {
                clicked = Some(h.id.clone());
            }
        }
    });
    clicked
}

/// The new-home choice (Characters' new-home row, lane 2B): the title and hint, then one row per
/// start in the file's order, each offered one with a radio, the one picked with its household
/// sub-choice, and each not offered greyed with why. Returns the choice as it stands after this
/// frame (`draft_choice`).
pub fn draw_choice(ui: &mut egui::Ui, theme: &Theme, starts: &Starts) -> Choice {
    heading(ui, theme);
    let picked = draft_choice(starts);
    for s in &starts.starts {
        ui.add_space(theme.spacing_sm);
        let name = row_name(starts, &s.id, &s.name);
        if !s.offered {
            ui.label(RichText::new(name).strong().color(theme.text_muted()));
            ui.indent(("where_greyed", &s.id), |ui| {
                row_lines(ui, theme, &s.sentence, Some(s), true);
                say(ui, theme, &s.not_offered_why, theme.text_muted());
            });
            continue;
        }
        let selected = picked.start == s.id;
        if ui.radio(selected, RichText::new(name).strong().color(theme.text_primary())).clicked() && !selected {
            DRAFT.with(|d| d.borrow_mut().start = Some(s.id.clone()));
        }
        ui.indent(("where_row", &s.id), |ui| {
            row_lines(ui, theme, &s.sentence, Some(s), false);
            if selected {
                if let Some(h) = household_radios(ui, theme, s, &picked.household) {
                    DRAFT.with(|d| d.borrow_mut().households.insert(s.id.clone(), h));
                }
            }
        });
    }
    draft_choice(starts)
}

/// A server's card (Characters, lane 2B), from that server's last `game_home` answer: where the
/// person lives there ("Where you live here: a Cabin, c2."), or, holding none, the choice with
/// each start's free count and a Choose button; the server's refusal sentence, or that the choice
/// was made; and the note that a server starts you fresh. `asking`: a request or a choice is on
/// its way (Choose waits for it). Returns the start and household whose Choose was pressed.
///
/// The starts' names and sentences, and why one is not offered, are the server's own; this game's
/// starts file only adds, for a start of the same id, what it shares aboard and the food and
/// water line, which the answer does not carry.
pub fn draw_server(ui: &mut egui::Ui, theme: &Theme, starts: &Starts, answer: Option<&HomeAnswer>, asking: bool) -> Option<Choice> {
    let mut chosen = None;
    match answer {
        None => say(ui, theme, if asking { ASKING } else { NO_ANSWER }, theme.text_secondary()),
        Some(a) => {
            if let Some(line) = held_line(starts, a) {
                ui.label(RichText::new(line).strong().color(theme.text_primary()));
                let start = a.plot.as_ref().map(|p| p.start.as_str()).unwrap_or("");
                let sentence = a.starts.iter().find(|o| o.id == start).map(|o| o.sentence.clone()).or_else(|| starts.get(start).map(|s| s.sentence.clone()));
                if let Some(sentence) = sentence.filter(|s| !s.is_empty()) {
                    say(ui, theme, &sentence, theme.text_secondary());
                }
            } else {
                ui.label(RichText::new(NO_HOME_HERE).strong().color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);
                heading(ui, theme);
                if a.starts.is_empty() && a.refused.is_none() {
                    say(ui, theme, NONE_OFFERED, theme.text_secondary());
                }
                for offer in &a.starts {
                    ui.add_space(theme.spacing_sm);
                    let local = starts.get(&offer.id);
                    let name = row_name(starts, &offer.id, &offer.name);
                    if !offer.offered {
                        ui.label(RichText::new(name).strong().color(theme.text_muted()));
                        ui.indent(("where_server_greyed", &offer.id), |ui| {
                            row_lines(ui, theme, &offer.sentence, local, true);
                            if !offer.why_not.is_empty() {
                                say(ui, theme, &offer.why_not, theme.text_muted());
                            }
                        });
                        continue;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(name).strong().color(theme.text_primary()));
                        let free_color = if offer.free > 0 { theme.success() } else { theme.warning() };
                        ui.label(RichText::new(free_text(offer.free)).size(theme.font_size_small).color(free_color));
                        if Button::secondary(CHOOSE).size(widgets::ButtonSize::Small).disabled(offer.free == 0 || asking).show(ui, theme) {
                            let household = local.map(|_| draft_household(starts, &offer.id)).unwrap_or_default();
                            chosen = Some(Choice { start: offer.id.clone(), household });
                        }
                    });
                    ui.indent(("where_server_row", &offer.id), |ui| {
                        row_lines(ui, theme, &offer.sentence, local, false);
                        // Who lives there, for a start that can be chosen now (none free: moot).
                        if let Some(s) = local.filter(|_| offer.free > 0) {
                            if let Some(h) = household_radios(ui, theme, s, &draft_household(starts, &s.id)) {
                                DRAFT.with(|d| d.borrow_mut().households.insert(s.id.clone(), h));
                            }
                        }
                    });
                }
            }
            if a.refused.is_some() {
                ui.add_space(theme.spacing_xs);
                let line = if a.message.is_empty() { REFUSED_NO_WORDS } else { a.message.as_str() };
                say(ui, theme, line, theme.warning());
            } else if a.chose && a.plot.is_some() {
                ui.add_space(theme.spacing_xs);
                say(ui, theme, CHOSEN, theme.success());
            }
            if asking {
                say(ui, theme, ASKING, theme.text_secondary());
            }
        }
    }
    ui.add_space(theme.spacing_xs);
    say(ui, theme, SERVER_NOTE, theme.text_muted());
    chosen
}

/// Settings > Gameplay, "Where you live" (drawn inside the section's card, in place of the old
/// "Home Design"): the live save's start and household, the household sub-choice when its start
/// has two, and where to go to live somewhere else.
pub fn draw_settings_section(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    draw_settings_section_with(ui, theme, state.settings.hint_display, starts());
}

/// [`draw_settings_section`] over a given starts file and hint mode.
pub(crate) fn draw_settings_section_with(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay, starts: &Starts) {
    ui.label(RichText::new(HEADING).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    let active = crate::save_load::active_home();
    ui.label(RichText::new(live_in_line(starts, &active.start, &active.household)).color(theme.text_primary()));
    let start = start_of(starts, &active.start);
    if start.households.len() > 1 {
        let current = starts.household(&start.id, &active.household).id.clone();
        if let Some(h) = household_radios(ui, theme, start, &current) {
            set_household(&h);
        }
        widgets::setting_hint(ui, theme, hint, TAKES_EFFECT);
    }
    widgets::setting_hint(ui, theme, hint, ELSEWHERE);
}

/// The Home page's "Where you live" card: the same line as Settings, read only.
pub fn draw_home_line(ui: &mut egui::Ui, theme: &Theme) {
    draw_home_line_with(ui, theme, starts());
}

/// [`draw_home_line`] over a given starts file.
pub(crate) fn draw_home_line_with(ui: &mut egui::Ui, theme: &Theme, starts: &Starts) {
    let active = crate::save_load::active_home();
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        widgets::subsection_label(ui, theme, HEADING);
        say(ui, theme, &live_in_line(starts, &active.start, &active.household), theme.text_primary());
    });
    ui.add_space(theme.spacing_sm);
}

#[cfg(test)]
#[path = "where_you_live_tests.rs"]
pub(crate) mod tests;
