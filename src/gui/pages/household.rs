//! Home > Household (ship homes increment 5b, lane 2B, 2026-10-12;
//! docs/design/ship-homes-increment-5b-plan.md section 3.9, with the amendments in its section 8,
//! which win where they differ): the card at the top of the Home page where a person gives a
//! friend a household permit for their plot on this server, renews one, or takes one back, and
//! sees the permits friends gave them.
//!
//! WHAT IT SHOWS.
//! - "Your plot": the plot this session's welcome gave (or why there is none), then one row per
//!   friend on this server (mutual follows, offline ones too) and anyone else holding a permit
//!   from me, each saying where their permit stands, with the buttons that fit: the lengths from
//!   data/ship/household.ron and Give a permit (which asks first), Renew, Give again, and Take
//!   back (which asks first). A row being taken back says "Taking it back..." until the server
//!   confirms.
//! - "Plots you may build on": each permit friends gave me on this server, and whether the server
//!   says it works this session.
//!
//! WHAT IT DOES NOT DO. Every action is one call into src/engine/household.rs (`give`, `renew`,
//! `take_back`), which returns the sentence to show when it cannot; the page never builds a
//! message itself. The rows are the engine's (`friends`, `held_rows`), so what the card says is
//! what the DM store holds, never what was clicked. The page's own state (the length picked per
//! friend, the one open question) lives in a `thread_local!`, the way homes.rs keeps its scale.
//!
//! WORDS. Plain ones only: the card never says certificate, signature, key, token, revoke or
//! verify (the plan's rule, and a test). Colours are theme tokens only.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Color32, RichText};

use crate::engine::household::{self as eh, FriendRow, HeldRow, Standing};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button};
use crate::gui::GuiState;
use crate::net::household::{self as hh, Lengths, Verdict};
use crate::systems::construction::shared::Why;

// ── The words ────────────────────────────────────────────────────────────────────────────────────

/// What a household permit is, first on the card (amendment A1: it can be taken back).
pub const INTRO: &str = "A household permit lets a friend build on your plot on this server, and walk into your home, \
     until the date you choose. Permits are kept on your devices; the server checks each one when it is used. \
     You can take a permit back at any time.";
/// The identity is locked.
pub const LOCKED: &str = "Unlock your identity to see your household permits.";
/// No server connected.
pub const NOT_CONNECTED: &str = "Connect to a server to see its household permits.";
/// Connected, but not in the server's shared world, so no plot is known this session.
pub const NOT_IN_WORLD: &str =
    "Step into this server's shared world to give permits: the server says which plot is yours when you arrive.";
/// In the shared world, the server's welcome not here yet (it names the plot, or none).
pub const JOINING: &str = "Joining this server's shared world...";
/// In the shared world with no plot of one's own.
pub const GUEST: &str = "You have no plot on this ship (the server had none free), so there is nothing to give a permit for.";
/// Nobody to list.
pub const NO_FRIENDS: &str = "No friends here yet: people you follow who follow you back on this server are listed here.";
/// A permit on its way to the server.
pub const SENDING: &str = "Sending the permit...";
/// A permit taken back that the server has not confirmed yet.
pub const TAKING_BACK: &str = "Taking it back...";
/// Nothing held.
pub const NONE_HELD: &str = "Nobody has given you a permit on this server.";
/// A held permit asked about, no answer yet.
pub const CHECKING: &str = "Checking it with the server...";
/// A held permit while not in the shared world (the server is asked when you arrive). It used to
/// say "It works in this server's shared world.", even for a permit the server had just said was
/// taken back (what it said is forgotten when you leave; 2026-10-12 review).
pub const WORKS_OUTSIDE: &str = "The server checks it when you step into its shared world.";
/// A held permit for the plot that is now your own here: nothing to check.
pub const YOUR_OWN_PLOT: &str = "That plot is your own on this ship now, so you need no permit for it.";

/// How long the card may go undrawn, seconds, before the lengths are read from the data file
/// again: once each time the card comes into view, never once per frame (an installed game whose
/// data folder predates the file would otherwise log "using the built-in copy" every frame).
const REREAD_AFTER_S: f64 = 1.0;
/// Seconds in a day.
const DAY: u64 = 86_400;

fn now_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// "1 day", "30 days".
pub fn days_words(days: u64) -> String {
    if days == 1 {
        "1 day".to_string()
    } else {
        format!("{days} days")
    }
}

/// "(29 days left)", "(1 day left)".
fn left_words(expiry: u64, now_s: u64) -> String {
    format!("({} left)", days_words(eh::days_left(expiry, now_s)))
}

/// The question asked before giving `name` a permit for `plot` running out at `expiry` (plan 3.9,
/// amendment A1: the no-take-back sentence is gone).
pub fn give_question(name: &str, plot: &str, expiry: u64) -> String {
    format!(
        "Give {name} a household permit for your plot {plot} until {}? {name} can build there and take down only \
         what {name} builds. You can take down anything on your plot. You can take it back at any time.",
        hh::date_words(expiry)
    )
}

/// The question asked before taking back `name`'s permit for `plot` (amendment A1).
pub fn take_back_question(name: &str, plot: &str) -> String {
    format!(
        "Take back {name}'s household permit for plot {plot}? {name} can no longer build there or walk in. \
         What {name} built stays; you can take it down."
    )
}

// ── The page's own state ─────────────────────────────────────────────────────────────────────────

/// The one question open on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Confirm {
    /// "Give Bo a household permit ...?" for `days` days.
    Give { key: String, days: u64 },
    /// "Take back Bo's household permit for plot p2?"
    TakeBack { key: String, plot: String },
}

/// What the card remembers between frames, and nothing more: never saved, never sent.
#[derive(Default)]
struct Draft {
    /// The length picked on each friend's row, days (the data file's default until one is picked).
    picked: BTreeMap<String, u64>,
    /// The question open, if any (one at a time).
    confirm: Option<Confirm>,
    /// The lengths as last read, and from which data folder.
    lengths: Option<(PathBuf, Lengths)>,
    /// When the card was last drawn (egui's clock, seconds).
    last_drawn: f64,
}

fn with_draft<R>(f: impl FnOnce(&mut Draft) -> R) -> R {
    thread_local! {
        static DRAFT: RefCell<Draft> = RefCell::new(Draft::default());
    }
    DRAFT.with(|d| f(&mut d.borrow_mut()))
}

/// The lengths a permit may be given for, from `data_dir`'s data/ship/household.ron (or the copy
/// built into the exe): read again whenever the card comes back into view after
/// [`REREAD_AFTER_S`], so an edit to the file shows the next time the page opens.
fn lengths(data_dir: &Path, now: f64) -> Lengths {
    with_draft(|d| {
        let away = now - d.last_drawn > REREAD_AFTER_S || now < d.last_drawn;
        d.last_drawn = now;
        match &d.lengths {
            Some((dir, l)) if dir == data_dir && !away => l.clone(),
            _ => {
                let l = hh::load_lengths(data_dir);
                d.lengths = Some((data_dir.to_path_buf(), l.clone()));
                l
            }
        }
    })
}

/// What a click asked for this frame, applied once the rows are drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Act {
    Pick(String, u64),
    AskGive(String, u64),
    Give(String, u64),
    Renew(String),
    AskTakeBack(String, String),
    TakeBack(String),
    Cancel,
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────────

/// The Household card, first on the Home page (homes.rs `draw`, with or without a design loaded):
/// a collapsible card. Its whole body shows: the Home page is one scroll since 2026-10-30 (the
/// operator: "We should join the two areas"), so the card's own scroll, capped at 360 points so
/// the rest of the page stayed in view (plan 3.9), went with the page's still top.
pub fn draw_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    draw_card_from(ui, theme, state, &crate::data_dir());
}

/// [`draw_card`] with the lengths read from `data_dir` (a test's own folder).
pub(crate) fn draw_card_from(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, data_dir: &Path) {
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        if widgets::section_disclosure(ui, theme, ("homes_sec", "household"), "Household", None) {
            let lengths = lengths(data_dir, ui.input(|i| i.time));
            draw_body(ui, theme, state, &lengths);
        }
    });
    ui.add_space(theme.spacing_sm);
}

fn say(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(color));
}

/// The card's body: the intro, then "Your plot" and "Plots you may build on".
fn draw_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, lengths: &Lengths) {
    widgets::body_hint(ui, theme, INTRO);
    ui.add_space(theme.spacing_xs);
    if state.private_key_bytes.is_none() || state.profile_public_key.is_empty() {
        say(ui, theme, LOCKED, theme.text_secondary());
        return;
    }
    // Permits are kept per server, so with no server there is nothing to list.
    if !state.ws_client.as_ref().is_some_and(|c| c.is_connected()) || !crate::engine::dm::ensure_dm_store(state) {
        say(ui, theme, NOT_CONNECTED, theme.text_secondary());
        return;
    }
    widgets::subsection_label(ui, theme, "Your plot");
    draw_your_plot(ui, theme, state, lengths);
    ui.add_space(theme.spacing_sm);
    widgets::subsection_label(ui, theme, "Plots you may build on");
    draw_held(ui, theme, state);
}

/// What each row needs besides itself.
struct RowCx<'a> {
    /// In the shared world with a plot of one's own: Give and Renew are offered.
    giving: bool,
    my_plot: Option<&'a str>,
    now: u64,
    lengths: &'a Lengths,
    confirm: Option<&'a Confirm>,
}

/// Does a row have anything to say outside a giving session (a permit given, an old plot, a
/// block)? Rows with nothing are listed only while permits can be given.
fn says_something(r: &FriendRow) -> bool {
    r.permit != Standing::None || !r.other_plots.is_empty() || r.blocked
}

/// The plot of the latest permit to `key` that might still stand: what Take back names when the
/// session has no plot of its own (outside the shared world).
fn standing_plot(state: &GuiState, key: &str, now: u64) -> Option<String> {
    let store = state.dm_store.as_ref()?;
    store
        .permits_given()
        .iter()
        .filter(|g| g.to == key && g.expiry > now && state.household.given_state(g).may_stand())
        .max_by_key(|g| g.at)
        .map(|g| g.plot.clone())
}

fn draw_your_plot(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, lengths: &Lengths) {
    let now = now_s();
    let in_world = state.copresence_active;
    let my_plot = state.household.my_plot.clone();
    // Under the protected setup the card offers no Give or Renew at all (they are refused anyway;
    // 2026-10-12 review: it offered them and refused only after the press). Take back stays.
    let protected = !state.protected.setup.permit_allowed();
    let giving = in_world && my_plot.is_some() && !protected;
    match (&my_plot, in_world) {
        (Some(p), true) => {
            ui.label(RichText::new(format!("Your plot on this server: {p}.")).size(theme.font_size_small).color(theme.text_primary()));
        }
        (None, true) if !state.household.welcomed => say(ui, theme, JOINING, theme.text_muted()),
        (None, true) => say(ui, theme, GUEST, theme.text_muted()),
        (_, false) => say(ui, theme, NOT_IN_WORLD, theme.text_muted()),
    }
    if protected && in_world && my_plot.is_some() {
        say(ui, theme, eh::PROTECTED, theme.text_muted());
    }
    ui.add_space(theme.spacing_xs);

    let rows = eh::friends(state);
    if rows.is_empty() {
        say(ui, theme, NO_FRIENDS, theme.text_muted());
    }
    let shown: Vec<FriendRow> = rows.into_iter().filter(|r| giving || says_something(r)).collect();
    // In a giving session every row's permit is for my plot; outside one, Take back names the plot
    // of the permit that might still stand.
    let plots: BTreeMap<String, String> = if my_plot.is_some() {
        BTreeMap::new()
    } else {
        shown.iter().filter_map(|r| standing_plot(state, &r.key, now).map(|p| (r.key.clone(), p))).collect()
    };
    let (confirm, picked) = with_draft(|d| (d.confirm.clone(), d.picked.clone()));
    let cx = RowCx { giving, my_plot: my_plot.as_deref(), now, lengths, confirm: confirm.as_ref() };
    let mut act = None;
    let mut busy = false;
    for (i, row) in shown.iter().enumerate() {
        if i > 0 {
            widgets::themed_separator(ui, theme);
        }
        let days = picked.get(&row.key).copied().filter(|d| lengths.lengths_days.contains(d)).unwrap_or(lengths.default_days);
        let plot = plots.get(&row.key).map(String::as_str);
        busy |= matches!(row.permit, Standing::Sending { .. } | Standing::TakingBack { .. });
        ui.push_id(("household_row", &row.key), |ui| draw_friend(ui, theme, row, &cx, days, plot, &mut act));
    }
    if busy {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
    if !state.household.line.is_empty() {
        ui.add_space(theme.spacing_xs);
        say(ui, theme, &state.household.line.clone(), theme.accent());
    }
    if let Some(a) = act {
        apply(state, a);
    }
}

/// One person on "Your plot": their name, where their permit stands, and the buttons that fit.
fn draw_friend(ui: &mut egui::Ui, theme: &Theme, row: &FriendRow, cx: &RowCx, days: u64, plot: Option<&str>, act: &mut Option<Act>) {
    let n = row.name.as_str();
    ui.label(RichText::new(n).size(theme.font_size_body).strong().color(theme.text_primary()));
    // Give and Renew go only to a friend, never to someone blocked, and only in a giving session.
    let may_give = cx.giving && row.friend && !row.blocked;
    let not_online = format!("{n} has not been online on this server since you started the app, so a permit cannot be sealed for them yet.");
    // A permit that might stand can be taken back.
    let standing = matches!(row.permit, Standing::Sending { .. } | Standing::Stands { .. } | Standing::Unconfirmed { .. });
    // Which give the row offers once its line is said: the lengths and a button with this label.
    let mut offer: Option<&str> = None;
    // One press, no question: Renew for N days, or Give again.
    let mut again: Option<String> = None;
    match &row.permit {
        Standing::None => offer = Some("Give a permit"),
        Standing::Sending { .. } => say(ui, theme, SENDING, theme.text_muted()),
        Standing::Stands { expiry, days } => {
            let text = format!("Can build on your plot until {} {}.", hh::date_words(*expiry), left_words(*expiry, cx.now));
            say(ui, theme, &text, theme.text_primary());
            again = Some(format!("Renew for {}", days_words(*days)));
        }
        Standing::Unconfirmed { .. } => {
            say(ui, theme, &format!("The server did not say whether {n} got the permit. Give it again to be sure."), theme.text_secondary());
            again = Some("Give again".to_string());
        }
        Standing::Refused { reason } => {
            say(ui, theme, &eh::refused_words(n, reason), theme.danger());
            offer = Some("Give a permit");
        }
        Standing::RanOut { expiry } => {
            say(ui, theme, &format!("Their permit ran out on {}.", hh::date_words(*expiry)), theme.text_secondary());
            offer = Some("Give a new one");
        }
        Standing::TakingBack { .. } => say(ui, theme, TAKING_BACK, theme.text_muted()),
        Standing::TakenBack { .. } => {
            say(ui, theme, "You took their permit back.", theme.text_secondary());
            offer = Some("Give a new one");
        }
    }
    let can_seal = may_give && row.sealable;
    if may_give && !row.sealable && (offer.is_some() || again.is_some()) {
        say(ui, theme, &not_online, theme.text_secondary());
    }

    // The open question, while it still fits this row (a permit taken back on another device, or
    // the session left, closes it here).
    let asking = cx.confirm.filter(|c| match c {
        Confirm::Give { key, .. } => key == &row.key && can_seal && offer.is_some(),
        Confirm::TakeBack { key, .. } => key == &row.key && standing,
    });
    match asking {
        Some(Confirm::Give { days, .. }) => {
            let plot = cx.my_plot.unwrap_or_default();
            say(ui, theme, &give_question(n, plot, cx.now + days * DAY), theme.text_primary());
            ui.horizontal_wrapped(|ui| {
                if Button::primary("Give").show(ui, theme) {
                    *act = Some(Act::Give(row.key.clone(), *days));
                }
                if Button::secondary("Cancel").show(ui, theme) {
                    *act = Some(Act::Cancel);
                }
            });
        }
        Some(Confirm::TakeBack { plot, .. }) => {
            say(ui, theme, &take_back_question(n, plot), theme.text_primary());
            ui.horizontal_wrapped(|ui| {
                if Button::danger("Take back").show(ui, theme) {
                    *act = Some(Act::TakeBack(row.key.clone()));
                }
                if Button::secondary("Cancel").show(ui, theme) {
                    *act = Some(Act::Cancel);
                }
            });
        }
        None => {
            if let Some(label) = offer.filter(|_| can_seal) {
                ui.horizontal_wrapped(|ui| {
                    for &d in &cx.lengths.lengths_days {
                        if Button::secondary(&days_words(d)).active(d == days).show(ui, theme) {
                            *act = Some(Act::Pick(row.key.clone(), d));
                        }
                    }
                    // A gap, so the action does not read as one more length.
                    ui.add_space(theme.spacing_md);
                    if Button::primary(label).show(ui, theme) {
                        *act = Some(Act::AskGive(row.key.clone(), days));
                    }
                });
            }
            let renew = again.as_deref().filter(|_| can_seal);
            if renew.is_some() || standing {
                ui.horizontal_wrapped(|ui| {
                    if let Some(label) = renew {
                        if Button::secondary(label).show(ui, theme) {
                            *act = Some(Act::Renew(row.key.clone()));
                        }
                    }
                    if standing && Button::secondary("Take back").show(ui, theme) {
                        let plot = cx.my_plot.or(plot).unwrap_or_default().to_string();
                        *act = Some(Act::AskTakeBack(row.key.clone(), plot));
                    }
                });
            }
        }
    }
    for (old, _) in &row.other_plots {
        say(ui, theme, &format!("For plot {old}, which is no longer yours here: it does not work any more."), theme.text_muted());
    }
    if row.blocked {
        say(
            ui,
            theme,
            &format!(
                "You blocked {n}: that takes back every permit you gave them, and none can be given until you unblock them. \
                 What {n} built stays; you can take it down."
            ),
            theme.text_muted(),
        );
    }
}

/// Apply this frame's click: a pick or a question stays on the card; Give, Renew and Take back
/// go to the engine once, and its sentence shows when it could not.
fn apply(state: &mut GuiState, act: Act) {
    let done = match act {
        Act::Pick(key, d) => {
            with_draft(|dr| dr.picked.insert(key, d));
            Ok(())
        }
        Act::AskGive(key, days) => {
            with_draft(|dr| dr.confirm = Some(Confirm::Give { key, days }));
            Ok(())
        }
        Act::AskTakeBack(key, plot) => {
            with_draft(|dr| dr.confirm = Some(Confirm::TakeBack { key, plot }));
            Ok(())
        }
        Act::Cancel => {
            with_draft(|dr| dr.confirm = None);
            Ok(())
        }
        Act::Give(key, days) => {
            with_draft(|dr| dr.confirm = None);
            eh::give(state, &key, days)
        }
        Act::Renew(key) => eh::renew(state, &key),
        Act::TakeBack(key) => {
            with_draft(|dr| dr.confirm = None);
            eh::take_back(state, &key).map(|_| ())
        }
    };
    if let Err(sentence) = done {
        state.household.line = sentence;
    }
}

/// How a held permit's line reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Plain,
    Good,
    Bad,
}

/// The line under a permit I hold (plan 3.9 "Plots you may build on", amendment A1 for one taken
/// back): `in_world` is whether this session is in the server's shared world, `on_ship` whether
/// the plot is one of this ship's.
fn held_line(r: &HeldRow, in_world: bool, on_ship: bool, my_plot: Option<&str>) -> (String, Tone) {
    let (n, p) = (r.name.as_str(), r.plot.as_str());
    if !on_ship {
        return (format!("Plot {p} is not on this ship any more."), Tone::Plain);
    }
    // Never asked about (engine/household.rs `tick` skips my own plot), so it said "Checking it
    // with the server..." for ever (2026-10-12 review).
    if in_world && my_plot == Some(p) {
        return (YOUR_OWN_PLOT.to_string(), Tone::Plain);
    }
    if r.ran_out || r.verdict == Some(Verdict::Refused(Why::PermitExpired)) {
        return (format!("Ran out on {}. Ask {n} for a new one.", hh::date_words(r.expiry)), Tone::Plain);
    }
    match r.verdict {
        Some(Verdict::Refused(Why::PermitNotFromHolder)) => (format!("It no longer works: {n} does not hold plot {p} any more."), Tone::Bad),
        Some(Verdict::Refused(Why::PermitWithdrawn)) => (format!("{n} took this permit back."), Tone::Bad),
        Some(Verdict::Refused(_)) => (format!("It does not check out. Ask {n} for a new one."), Tone::Bad),
        _ if !in_world => (WORKS_OUTSIDE.to_string(), Tone::Plain),
        Some(Verdict::Works) => (
            format!("The server says it works: plot {p}'s door opens for you, and you can build there and take down what you build."),
            Tone::Good,
        ),
        Some(Verdict::Checking | Verdict::Later) | None => (CHECKING.to_string(), Tone::Plain),
    }
}

fn draw_held(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    let rows = eh::held_rows(state);
    if rows.is_empty() {
        say(ui, theme, NONE_HELD, theme.text_muted());
        return;
    }
    let now = now_s();
    let in_world = state.copresence_active;
    // A ship not loaded yet says nothing about which plots it has.
    let on_ship = |plot: &str| state.ship_structure.as_ref().is_none_or(|s| s.plots.iter().any(|p| p.id == plot));
    let mut checking = false;
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            ui.add_space(theme.spacing_xs);
        }
        let heading = if r.ran_out {
            format!("{}'s plot {}.", r.name, r.plot)
        } else {
            format!("{}'s plot {}, until {} {}.", r.name, r.plot, hh::date_words(r.expiry), left_words(r.expiry, now))
        };
        ui.label(RichText::new(heading).size(theme.font_size_small).strong().color(theme.text_primary()));
        let (line, tone) = held_line(r, in_world, on_ship(&r.plot), state.household.my_plot.as_deref());
        checking |= line == CHECKING;
        let color = match tone {
            Tone::Plain => theme.text_secondary(),
            Tone::Good => theme.success(),
            Tone::Bad => theme.danger(),
        };
        say(ui, theme, &line, color);
    }
    if checking {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
}

#[cfg(test)]
#[path = "household_tests.rs"]
pub(crate) mod tests;
