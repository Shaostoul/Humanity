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
//! THE KINDS OF HOME AS CARDS (2026-11-02, docs/design/top-menu-and-game-pages.md section 5; the
//! operator: "The left and right bar areas are small so a lot of text is crammed vertically"). In
//! Characters each kind of home is a card, side by side as the width allows (`widgets::cards::Grid`):
//! its name, how many are free (on a server), its first sentence, the household choice on the card
//! it belongs to, Choose, and a "What it is like" fold whose words (the whole sentence, the places
//! shared aboard, food, water and power) open in a panel under the row at a readable width. They
//! were one long column of paragraphs.
//!
//! WORDS. Every start's name and sentence, and why a start is not offered yet, comes from
//! data/homes/starts.ron (`Starts`) or, on a server's card, from that server's own answer
//! (`HomeAnswer`), never from a copy here. The shared places are the zone types' own labels
//! (data/blueprints/zone_types.ron). The words this file owns are about the choice itself, and come
//! from data/i18n/en.json by key (`where_you_live.*`): its title and hint, what each way of looking
//! after food, water and power means (`engagement_line`), and the server card's sentences. None of
//! them, and no start's words (a test holds both), says tier, rank, level or berth: where you live
//! is how much space you hold and how much looking after it needs, never a step on a ladder
//! (docs/design/engagement-modes.md).
//!
//! THE HOUSEHOLD IS THE SAVE'S. Choosing "Just you" in Settings changes the live save's household
//! (`persistence::live_save::ActiveHome::household`), which the save writes at its next save and
//! the household file the world loads comes from (`machines::home_ron_path`). It never touches
//! AppConfig: since increment 6 each save keeps its own, and a test holds that too.
//!
//! The widget's own choices (the start picked on the new-home row, the household picked per start)
//! live in a `thread_local!`, the way homes.rs keeps its scale. The website's mirror of the
//! server's card is web/chat/chat-home.js. Colours are theme tokens only.

use std::cell::RefCell;
use std::collections::BTreeMap;

use egui::{Color32, RichText};
use humanity_core::i18n::{t, tf, tn};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cards, Button, Hint};
use crate::gui::{GuiState, HintDisplay};
use humanity_systems::persistence::SaveSlot;
use humanity_ship::starts::{Engagement, EngagementDefaults, HomeAnswer, Start, Starts};

/// The narrowest a kind of home's card is laid out at before its row loses a column: three across
/// in the operator's WHERE column (about 850 points), two on the app's restored window.
pub const KIND_CARD_MIN: f32 = 220.0;

// ── The words ────────────────────────────────────────────────────────────────────────────────────

/// What food is, for each way of looking after it.
fn food_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => t("where_you_live.food_direct"),
        Engagement::Trade => t("where_you_live.food_trade"),
        Engagement::Automation => t("where_you_live.food_automation"),
        Engagement::Cooperative => t("where_you_live.food_cooperative"),
        Engagement::Background => t("where_you_live.food_background"),
    }
}

/// What water and power are, when both are looked after the same way.
fn water_and_power_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => t("where_you_live.water_power_direct"),
        Engagement::Trade => t("where_you_live.water_power_trade"),
        Engagement::Automation => t("where_you_live.water_power_automation"),
        Engagement::Cooperative => t("where_you_live.water_power_cooperative"),
        Engagement::Background => t("where_you_live.water_power_background"),
    }
}

/// What water is, when power is looked after another way.
fn water_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => t("where_you_live.water_direct"),
        Engagement::Trade => t("where_you_live.water_trade"),
        Engagement::Automation => t("where_you_live.water_automation"),
        Engagement::Cooperative => t("where_you_live.water_cooperative"),
        Engagement::Background => t("where_you_live.water_background"),
    }
}

/// What power is, when water is looked after another way.
fn power_words(e: Engagement) -> &'static str {
    match e {
        Engagement::Direct => t("where_you_live.power_direct"),
        Engagement::Trade => t("where_you_live.power_trade"),
        Engagement::Automation => t("where_you_live.power_automation"),
        Engagement::Cooperative => t("where_you_live.power_cooperative"),
        Engagement::Background => t("where_you_live.power_background"),
    }
}

// ── Words from data ──────────────────────────────────────────────────────────────────────────────

/// The starts file, read once per run: the widget is drawn every frame the page is open.
pub fn starts() -> &'static Starts {
    static STARTS: std::sync::OnceLock<Starts> = std::sync::OnceLock::new();
    STARTS.get_or_init(|| Starts::load(&humanity_core::storage::data_dir()))
}

/// "a Cabin", "an Apartment": a name as written, with its article. (English's article rule: a
/// translation that needs another one replaces the sentences that use it, `live_in` and `held`.)
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
        tf("where_you_live.row_default", &[("name", &name)])
    } else {
        name.to_string()
    }
}

/// A start's sentence as its card shows it: the first sentence, whole ("One private room of your
/// own."); the rest is in the card's fold. A sentence of one sentence is all of it.
pub fn first_sentence(sentence: &str) -> &str {
    let s = sentence.trim();
    match s.find(". ") {
        Some(end) => &s[..=end],
        None => s,
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
            let label = humanity_ship::structure::zone_type(id).map(|z| z.label.clone()).unwrap_or_else(|| id.replace(['_', '-'], " "));
            tf("where_you_live.shared_place", &[("place", &mid_sentence(&label))])
        })
        .collect();
    Some(tf("where_you_live.shared", &[("places", &names.join(", "))]))
}

/// One plain line from a start's engagement: "Food: free crew meals in the mess hall. Water and
/// power: the ship's, counted on your meter." Water and power are said together when they are
/// looked after the same way.
pub fn engagement_line(e: &EngagementDefaults) -> String {
    let food = tf("where_you_live.food_line", &[("what", &food_words(e.food))]);
    if e.water == e.energy {
        let wp = tf("where_you_live.water_power_line", &[("what", &water_and_power_words(e.water))]);
        format!("{food} {wp}")
    } else {
        let w = tf("where_you_live.water_line", &[("what", &water_words(e.water))]);
        let p = tf("where_you_live.power_line", &[("what", &power_words(e.energy))]);
        format!("{food} {w} {p}")
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
    tf("where_you_live.live_in", &[("home", &a_or_an(&s.name)), ("household", &mid_sentence(&h.name))])
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
    Some(tf("where_you_live.held", &[("home", &a_or_an(&name)), ("plot", &held.id)]))
}

/// "12 free", "1 free", "none free".
pub fn free_text(n: u32) -> String {
    if n > 0 {
        tn("where_you_live.free", n as i64, &[])
    } else {
        t("where_you_live.none_free").to_string()
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
    let mut active = humanity_systems::persistence::live_save::active_home();
    active.household = household.to_string();
    humanity_systems::persistence::live_save::set_active_home(active);
}

// ── Asking the server (the save slot's questions on the GUI state) ───────────────────────────────
//
// The picker's server card asks these on the GUI state alone; they were engine/save_slot.rs's until
// the crate split's rung 0, part B (2026-10-31). What the answers do to the saves is still the
// engine's (engine/save_slot.rs `on_game_message`, `follow`).

/// The save this identity keeps for the server the game talks to, whatever the pairing says (a
/// server card's Choose is made while paired with your own home). None without an identity or a
/// server.
pub fn server_slot(gui: &GuiState) -> Option<SaveSlot> {
    match humanity_systems::persistence::live_save::slot_for("", &gui.profile_public_key, &gui.active_server_key(), false) {
        SaveSlot::Offline => None,
        slot => Some(slot),
    }
}

/// Ask the server the game talks to where this identity lives on it, and what is free
/// (`game_home_request`; the answer comes to engine/save_slot.rs `on_game_message`). False when
/// not connected.
pub fn ask_where_you_live(gui: &GuiState) -> bool {
    match gui.ws_client.as_ref() {
        Some(ws) if ws.is_connected() && gui.ws_identified => {
            ws.send(&serde_json::json!({ "type": humanity_ship::starts::HOME_REQUEST }).to_string());
            true
        }
        _ => false,
    }
}

thread_local! {
    /// The start and household a server card's Choose asked for, until its answer comes
    /// (engine/save_slot.rs `take_answer` takes it).
    static ASKED: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

/// What the last Choose asked for, taken once (engine/save_slot.rs `take_answer`, when the answer
/// comes).
pub fn take_asked() -> Option<(String, String)> {
    ASKED.with(|a| a.borrow_mut().take())
}

/// A server card's Choose (3.9; the probe's `{"choose_start":"cabin"}` through engine/save_slot.rs
/// `choose_on_server`): send `game_choose_home` for `start` to the server the game talks to, with
/// the household the card picked for it (`draft_household`). Its answer (engine/save_slot.rs
/// `on_game_message`) puts the home chosen into that server's save. Err with the sentence to show
/// when it cannot be asked.
pub fn choose_on_server(gui: &GuiState, start: &str) -> Result<(), String> {
    let starts = starts();
    let Some(row) = starts.get(start).filter(|s| s.offered) else {
        return Err(tf("where_you_live.not_a_choice", &[("start", &start)]));
    };
    if server_slot(gui).is_none() {
        return Err(t("where_you_live.needs_identity").to_string());
    }
    let Some(ws) = gui.ws_client.as_ref().filter(|w| w.is_connected() && gui.ws_identified) else {
        return Err(t("where_you_live.connect_first").to_string());
    };
    ws.send(&serde_json::json!({ "type": humanity_ship::starts::CHOOSE_HOME, "start": row.id }).to_string());
    let household = draft_household(starts, &row.id);
    ASKED.with(|a| *a.borrow_mut() = Some((row.id.clone(), household)));
    log::info!("Where you live: asked this server for a {}", row.id);
    Ok(())
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────────

fn say(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(color));
}

/// The title and its hint.
fn heading(ui: &mut egui::Ui, theme: &Theme) {
    ui.label(RichText::new(t("where_you_live.title")).size(theme.font_size_body).strong().color(theme.text_primary()));
    say(ui, theme, t("where_you_live.hint"), theme.text_muted());
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
            if ui
                .radio(selected, RichText::new(&h.name).size(theme.font_size_small).color(theme.text_primary()))
                .hint("where_you_live.household_hint")
                .clicked()
                && !selected
            {
                clicked = Some(h.id.clone());
            }
        }
    });
    clicked
}

/// Where a kind-of-home card is drawn: the new-home row's choice, or a server's card. Names its
/// folds apart, so opening one place's "What it is like" never opens the other's.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Place {
    Choice,
    Server,
}

/// The id of the "What it is like" fold of start `id` on `place`'s cards (a snapshot or a test
/// opens it with `cards::set_fold_open`).
pub fn fold_salt(place: Place, id: &str) -> (&'static str, Place, String) {
    ("where_you_live_kind", place, id.to_string())
}

/// One kind of home, as its card shows it.
struct Kind<'a> {
    id: &'a str,
    /// Its name on the card (`row_name`).
    name: String,
    /// The whole sentence: the server's on a server's card, else this game's.
    sentence: &'a str,
    /// This game's row of the same start, for the lines a server's answer does not carry.
    local: Option<&'a Start>,
    offered: bool,
    why_not: &'a str,
}

/// The card's top: the name (with `before` drawn in front of it on the same line, a radio or
/// nothing), the first sentence, and why it is not offered, greyed when it cannot be chosen.
fn card_words(ui: &mut egui::Ui, theme: &Theme, kind: &Kind, title: impl FnOnce(&mut egui::Ui, &str)) {
    title(ui, &kind.name);
    let main = if kind.offered { theme.text_secondary() } else { theme.text_muted() };
    let first = first_sentence(kind.sentence);
    if !first.is_empty() {
        say(ui, theme, first, main);
    }
    if !kind.offered && !kind.why_not.is_empty() {
        say(ui, theme, kind.why_not, theme.text_muted());
    }
}

/// A card's "What it is like" fold. Returns whether it is open.
fn card_fold(ui: &mut egui::Ui, theme: &Theme, place: Place, kind: &Kind) -> bool {
    ui.add_space(theme.spacing_xs);
    cards::fold(ui, theme, fold_salt(place, kind.id), t("where_you_live.more"), "where_you_live.more_hint")
}

/// The panel under a card's row when its fold is open: the whole sentence, the places shared
/// aboard, and how its food, water and power are looked after, at a readable width.
fn card_detail(ui: &mut egui::Ui, theme: &Theme, kind: &Kind) {
    cards::detail_heading(ui, theme, &kind.name, t("where_you_live.more"));
    cards::readable(ui, theme, |ui| {
        if !kind.sentence.is_empty() {
            say(ui, theme, kind.sentence, theme.text_secondary());
        }
        if let Some(s) = kind.local {
            if let Some(line) = shared_line(&s.shared) {
                say(ui, theme, &line, theme.text_muted());
            }
            say(ui, theme, &engagement_line(&s.engagement), theme.text_muted());
        }
        if !kind.offered && !kind.why_not.is_empty() {
            say(ui, theme, kind.why_not, theme.text_muted());
        }
    });
}

/// The kinds of home as cards side by side (`cards::Grid`), each card drawn by `card`, which
/// returns whether its fold is open; an open fold's words go in a panel under its row.
fn kind_grid(ui: &mut egui::Ui, theme: &Theme, place: Place, kinds: &[Kind], mut card: impl FnMut(&mut egui::Ui, &Kind) -> bool) {
    cards::Grid::new(("where_you_live_kinds", place))
        .min_width(KIND_CARD_MIN)
        .max_columns(kinds.len().max(1))
        .gap(theme.spacing_sm)
        .show_with_details(ui, theme, kinds.len(), |ui, i| card(ui, &kinds[i]), |ui, i| card_detail(ui, theme, &kinds[i]));
}

/// The new-home choice (Characters' new-home row, lane 2B): the title and hint, then one card per
/// start in the file's order, each offered one with a radio on its name, the one picked with its
/// household sub-choice, and each not offered greyed with why. Returns the choice as it stands
/// after this frame (`draft_choice`). `dev`: the Dev play mode, where the drafts for developers
/// (`Start::dev`, the room program's homesteads) are listed too; outside it they are not shown.
pub fn draw_choice(ui: &mut egui::Ui, theme: &Theme, starts: &Starts, dev: bool) -> Choice {
    heading(ui, theme);
    ui.add_space(theme.spacing_sm);
    let picked = draft_choice(starts);
    let kinds: Vec<Kind> = starts
        .starts
        .iter()
        .filter(|s| dev || !s.dev)
        .map(|s| Kind {
            id: &s.id,
            name: row_name(starts, &s.id, &s.name),
            sentence: &s.sentence,
            local: Some(s),
            offered: s.offered,
            why_not: &s.not_offered_why,
        })
        .collect();
    kind_grid(ui, theme, Place::Choice, &kinds, |ui, kind| {
        let selected = picked.start == kind.id;
        card_words(ui, theme, kind, |ui, name| {
            if !kind.offered {
                ui.label(RichText::new(name).strong().color(theme.text_muted()));
            } else if ui.radio(selected, RichText::new(name).strong().color(theme.text_primary())).hint("where_you_live.pick_hint").clicked() && !selected {
                DRAFT.with(|d| d.borrow_mut().start = Some(kind.id.to_string()));
            }
        });
        if selected && kind.offered {
            if let Some(s) = kind.local {
                if let Some(h) = household_radios(ui, theme, s, &picked.household) {
                    DRAFT.with(|d| d.borrow_mut().households.insert(s.id.clone(), h));
                }
            }
        }
        card_fold(ui, theme, Place::Choice, kind)
    });
    draft_choice(starts)
}

/// A server's card (Characters, lane 2B), from that server's last `game_home` answer: where the
/// person lives there ("Where you live here: a Cabin, c2."), or, holding none, the kinds of home
/// as cards with each one's free count and a Choose button; the server's refusal sentence, or that
/// the choice was made; and the note that a server starts you fresh. `asking`: a request or a
/// choice is on its way (Choose waits for it). Returns the start and household whose Choose was
/// pressed.
///
/// The starts' names and sentences, and why one is not offered, are the server's own; this game's
/// starts file only adds, for a start of the same id, what it shares aboard and the food and
/// water line, which the answer does not carry.
pub fn draw_server(ui: &mut egui::Ui, theme: &Theme, starts: &Starts, answer: Option<&HomeAnswer>, asking: bool) -> Option<Choice> {
    let mut chosen = None;
    match answer {
        None => say(ui, theme, if asking { t("where_you_live.asking") } else { t("where_you_live.no_answer") }, theme.text_secondary()),
        Some(a) => {
            if let Some(line) = held_line(starts, a) {
                ui.label(RichText::new(line).strong().color(theme.text_primary()));
                let start = a.plot.as_ref().map(|p| p.start.as_str()).unwrap_or("");
                let sentence = a.starts.iter().find(|o| o.id == start).map(|o| o.sentence.clone()).or_else(|| starts.get(start).map(|s| s.sentence.clone()));
                if let Some(sentence) = sentence.filter(|s| !s.is_empty()) {
                    say(ui, theme, &sentence, theme.text_secondary());
                }
            } else {
                ui.label(RichText::new(t("where_you_live.no_home_here")).strong().color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);
                heading(ui, theme);
                if a.starts.is_empty() && a.refused.is_none() {
                    say(ui, theme, t("where_you_live.none_offered"), theme.text_secondary());
                }
                ui.add_space(theme.spacing_sm);
                let kinds: Vec<Kind> = a
                    .starts
                    .iter()
                    .map(|o| Kind {
                        id: &o.id,
                        name: row_name(starts, &o.id, &o.name),
                        sentence: &o.sentence,
                        local: starts.get(&o.id),
                        offered: o.offered,
                        why_not: &o.why_not,
                    })
                    .collect();
                kind_grid(ui, theme, Place::Server, &kinds, |ui, kind| {
                    let free = a.starts.iter().find(|o| o.id == kind.id).map(|o| o.free).unwrap_or(0);
                    card_words(ui, theme, kind, |ui, name| {
                        ui.horizontal_wrapped(|ui| {
                            let ink = if kind.offered { theme.text_primary() } else { theme.text_muted() };
                            ui.label(RichText::new(name).strong().color(ink));
                            if kind.offered {
                                let free_color = if free > 0 { theme.success() } else { theme.warning() };
                                ui.label(RichText::new(free_text(free)).size(theme.font_size_small).color(free_color));
                            }
                        });
                    });
                    if kind.offered {
                        // Who lives there, for a start that can be chosen now (none free: moot).
                        if let Some(s) = kind.local.filter(|_| free > 0) {
                            if let Some(h) = household_radios(ui, theme, s, &draft_household(starts, &s.id)) {
                                DRAFT.with(|d| d.borrow_mut().households.insert(s.id.clone(), h));
                            }
                        }
                        if Button::secondary(t("where_you_live.choose"))
                            .size(widgets::ButtonSize::Small)
                            .disabled(free == 0 || asking)
                            .hint("where_you_live.choose_hint")
                            .show(ui, theme)
                        {
                            let household = kind.local.map(|_| draft_household(starts, kind.id)).unwrap_or_default();
                            chosen = Some(Choice { start: kind.id.to_string(), household });
                        }
                    }
                    card_fold(ui, theme, Place::Server, kind)
                });
            }
            if a.refused.is_some() {
                ui.add_space(theme.spacing_xs);
                let line = if a.message.is_empty() { t("where_you_live.refused_no_words") } else { a.message.as_str() };
                say(ui, theme, line, theme.warning());
            } else if a.chose && a.plot.is_some() {
                ui.add_space(theme.spacing_xs);
                say(ui, theme, t("where_you_live.chosen"), theme.success());
            }
            if asking {
                say(ui, theme, t("where_you_live.asking"), theme.text_secondary());
            }
        }
    }
    ui.add_space(theme.spacing_xs);
    say(ui, theme, t("where_you_live.server_note"), theme.text_muted());
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
    ui.label(RichText::new(t("where_you_live.heading")).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    let active = humanity_systems::persistence::live_save::active_home();
    ui.label(RichText::new(live_in_line(starts, &active.start, &active.household)).color(theme.text_primary()));
    let start = start_of(starts, &active.start);
    if start.households.len() > 1 {
        let current = starts.household(&start.id, &active.household).id.clone();
        if let Some(h) = household_radios(ui, theme, start, &current) {
            set_household(&h);
        }
        widgets::setting_hint(ui, theme, hint, t("where_you_live.takes_effect"));
    }
    widgets::setting_hint(ui, theme, hint, t("where_you_live.elsewhere"));
}

/// The Home page's "Where you live" card: the same line as Settings, read only.
pub fn draw_home_line(ui: &mut egui::Ui, theme: &Theme) {
    draw_home_line_with(ui, theme, starts());
}

/// [`draw_home_line`] over a given starts file.
pub(crate) fn draw_home_line_with(ui: &mut egui::Ui, theme: &Theme, starts: &Starts) {
    let active = humanity_systems::persistence::live_save::active_home();
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        widgets::subsection_label(ui, theme, t("where_you_live.heading"));
        say(ui, theme, &live_in_line(starts, &active.start, &active.household), theme.text_primary());
    });
    ui.add_space(theme.spacing_sm);
}

#[cfg(test)]
#[path = "where_you_live_tests.rs"]
pub(crate) mod tests;
