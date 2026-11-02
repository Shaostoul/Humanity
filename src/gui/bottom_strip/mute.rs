//! Muting the unfinished-software warning for a while (2026-11-02, docs/design/app-bottom-strip.md,
//! section 3.5).
//!
//! The operator, 2026-11-02, answering the plan's question 1 (can the warning be dismissed?): "We
//! could have like a "mute for this session" button in the warning bar. What do you think about like
//! mute for this session,  one day, one week?" And question 2: "It should stay on the title. I think
//! it helps keep it honest."
//!
//! THE RULE, [`muted`], held to one list of cases with the website's `hosWipMuted`
//! (scripts/tests/fixtures/wip-mute-cases.json): a mute holds only on the version it was made on
//! (what is unfinished changes with every version, so a new version shows the bar again) and only
//! before its time. A mute of this session has no time and is never written to config.json
//! ([`kept`]), so the next launch shows the bar. Anything it cannot read shows the bar: failing
//! toward the warning is the safe direction.
//!
//! WHERE NO MUTE APPLIES, [`always_here`]: the title screen (his answer 2), and the warning's own
//! page in the Library, so the bar is there while someone reads about it. There a mute in force
//! reads "Muted", and its menu says until when, with Show it again.
//!
//! THE CHOICES are data, `data/gui/wip_mute.json` (the website reads the same file): each is this
//! session or a number of seconds, with its words by key.
//!
//! KEPT on this device only: `GuiState::wip_mute`, and config.json's `wip_mute` for a day or a week.
//! It is a choice the person made, kept as a setting, not a cache. Settings > Notifications > Muted
//! always says whether the bar is muted and until when, with Show it again ([`settings_row`]).
//!
//! COST: comparing a number and a string on frames the app already draws; the menu's choices are
//! read from the data file once for the run, when first asked for.

use std::sync::OnceLock;

use egui::{RichText, Sense};
use serde::{Deserialize, Serialize};

use humanity_core::i18n::{t, t_dyn, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiPage, GuiState, VERSION};

/// The choices, under data/.
const REL: &str = "gui/wip_mute.json";

/// The bar's menu, by id (egui keeps whether it is open).
const MENU: &str = "hos_wip_mute_menu";

/// A mute of the warning bar: kept in `GuiState`, and in config.json when it has a time. Read back
/// with [`read_kept`], never a plain derive, so a record this version cannot read is no mute
/// rather than a config.json that fails to load whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WipMute {
    /// Until when, in seconds since 1970 (UTC). `None`: until the app is closed (this session).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
    /// The version it was muted on: a mute holds on that version only.
    pub version: String,
}

/// A kept mute record, as config.json (or the shared test cases) hold it: an object with a
/// `version` string and, unless it is this session's, an `until` in whole seconds. Anything else
/// is no mute, so the bar shows (the website's `hosWipMuted` reads its record the same way).
pub fn from_record(v: &serde_json::Value) -> Option<WipMute> {
    let o = v.as_object()?;
    let version = o.get("version")?.as_str()?.to_string();
    let until = match o.get("until") {
        None | Some(serde_json::Value::Null) => None,
        Some(u) => Some(u.as_i64()?),
    };
    Some(WipMute { until, version })
}

/// config.json's `wip_mute`, read leniently through [`from_record`]: a hand-edited or damaged
/// record is no mute; it never makes the whole file fail to load (which would put every setting
/// back to its default).
pub fn read_kept<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<WipMute>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(from_record(&v))
}

/// One way to mute it, from the data file.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub id: String,
    /// Until the app is closed.
    #[serde(default)]
    pub session: bool,
    /// How long from the moment it is chosen.
    #[serde(default)]
    pub seconds: Option<i64>,
    /// Its words and its hint, keys in data/i18n/en.json.
    pub label: String,
    pub hint: String,
}

/// `data/gui/wip_mute.json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct File {
    #[serde(default)]
    pub mute_for: Vec<Choice>,
}

impl File {
    /// The file's text, read; a choice that is neither this session nor a time above zero is refused,
    /// so a typo cannot make a mute that never ends.
    pub fn parse(text: &str) -> Result<File, String> {
        let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
        for c in &file.mute_for {
            let timed = c.seconds.is_some_and(|s| s > 0);
            if c.session == timed {
                return Err(format!("the choice {:?} must be either `session` or a number of `seconds` above 0", c.id));
            }
        }
        Ok(file)
    }
}

/// The choices, in order, read once for the run. None only when both the data folder's copy and
/// the exe's own fail to load (a lib test rules out the second): then the bar offers no mute.
pub fn choices() -> &'static [Choice] {
    static FILE: OnceLock<File> = OnceLock::new();
    &FILE
        .get_or_init(|| {
            humanity_core::embedded_data::load_text_or_embedded(&humanity_core::storage::data_dir(), REL, File::parse).unwrap_or_else(|e| {
                log::warn!("{e}: the warning bar offers no mute this run");
                File::default()
            })
        })
        .mute_for
}

/// Seconds since 1970, now.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Does `mute` hide the warning on `version` at `now`? Only on the version it was made on, and only
/// before its time (a session mute has none). The same rule as the website's `hosWipMuted`.
pub fn muted(mute: Option<&WipMute>, version: &str, now: i64) -> bool {
    let Some(m) = mute else { return false };
    if m.version.is_empty() || m.version != version {
        return false;
    }
    match m.until {
        None => true,
        Some(until) => now < until,
    }
}

/// The mute a choice makes, chosen at `now` on `version`.
pub fn mute_for(choice: &Choice, version: &str, now: i64) -> WipMute {
    let until = if choice.session { None } else { choice.seconds.map(|s| now.saturating_add(s)) };
    WipMute { until, version: version.to_string() }
}

/// What config.json keeps of a mute: one with a time. A session mute is never written, so the next
/// launch shows the bar; one found in the file all the same (written by hand) is not taken.
pub fn kept(mute: Option<&WipMute>) -> Option<WipMute> {
    mute.filter(|m| m.until.is_some()).cloned()
}

/// The pages that show the bar whatever the mute: the title screen (the operator, 2026-11-02: "It
/// should stay on the title. I think it helps keep it honest."), and the warning's own page in the
/// Library, so the bar is there while someone reads about it.
pub fn always_here(state: &GuiState) -> bool {
    match state.active_page {
        GuiPage::MainMenu => true,
        GuiPage::Library => crate::gui::pages::library::shown_doc(state).as_deref() == Some(super::DETAILS_SLUG),
        _ => false,
    }
}

/// "Muted until 2026-11-03 14:00", or "Muted until HumanityOS is closed".
pub fn muted_words(m: &WipMute) -> String {
    match m.until {
        None => t("wip.muted_session").to_string(),
        Some(until) => {
            let time = crate::gui::pages::chat::format_date_minute((until.max(0) as u64).saturating_mul(1000));
            tf("wip.muted_until", &[("time", &time)])
        }
    }
}

/// Mute it as `choice` says, and keep that (a session mute takes a kept one out of config.json).
pub fn choose(state: &mut GuiState, choice: &Choice, version: &str, now: i64) {
    state.wip_mute = Some(mute_for(choice, version, now));
    save(state);
}

/// Show it again now: the mute is gone, here and in config.json.
pub fn show_again(state: &mut GuiState) {
    state.wip_mute = None;
    save(state);
}

fn save(state: &GuiState) {
    crate::config::AppConfig::from_gui_state(state).save();
}

/// The bar's Mute, at `size`, and its menu, opening above the bar. On a page that shows the bar
/// whatever the mute ([`always_here`]), a mute in force reads "Muted", and the menu says until
/// when, with Show it again above the choices.
pub(crate) fn draw_control(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, version: &str, now: i64, size: f32) {
    let in_force = muted(state.wip_mute.as_ref(), version, now);
    let (label, hint) = if in_force { (t("wip.muted"), "wip.muted_hint") } else { (t("wip.mute"), "wip.mute_hint") };
    let menu = egui::Id::new(MENU);
    let open = ui
        .add(egui::Label::new(RichText::new(label).size(size).color(theme.warning()).underline()).sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .hint(hint);
    #[cfg(test)]
    tests::record("mute", open.rect);
    if open.clicked() {
        ui.memory_mut(|m| m.toggle_popup(menu));
    }
    let above = egui::AboveOrBelow::Above;
    let _ = egui::popup_above_or_below_widget(ui, menu, &open, above, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(14.0 * theme.font_size_body);
        let mut done = false;
        if let Some(m) = state.wip_mute.clone().filter(|m| muted(Some(m), version, now)) {
            ui.label(RichText::new(muted_words(&m)).size(theme.font_size_small).color(theme.text_secondary()));
            let again = ui.button(t("wip.show_again")).hint("wip.show_again_hint");
            #[cfg(test)]
            tests::record("show_again", again.rect);
            if again.clicked() {
                show_again(state);
                done = true;
            }
            ui.separator();
        }
        for c in choices() {
            let pick = ui.button(t_dyn(&c.label)).hint_dyn(&c.hint);
            #[cfg(test)]
            tests::record(&c.id, pick.rect);
            if pick.clicked() {
                choose(state, c, version, now);
                done = true;
            }
        }
        if done {
            ui.memory_mut(|m| m.close_popup());
        }
    });
}

/// Settings > Notifications > Muted: the warning bar's row. Whether it is muted and until when, with
/// Show it again, or the same choices as the bar's Mute (CLAUDE.md, GUI-first: whatever the bar does,
/// Settings can do and undo). Nothing once the warning has gone (version 1.0).
pub(crate) fn settings_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    settings_row_at(ui, theme, state, VERSION, now_secs());
}

/// [`settings_row`] for a given version and time (the tests and the snapshot pin both).
pub(crate) fn settings_row_at(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, version: &str, now: i64) {
    if !super::wip_shown(version) {
        return;
    }
    let hint = state.settings.hint_display;
    let small = theme.font_size_small;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(t("wip.settings_label")).color(theme.text_primary()));
        match state.wip_mute.clone().filter(|m| muted(Some(m), version, now)) {
            Some(m) => {
                ui.label(RichText::new(muted_words(&m)).size(small).color(theme.text_secondary()));
                let again = ui.scope(|ui| widgets::Button::secondary(t("wip.show_again")).hint("wip.show_again_hint").show(ui, theme));
                #[cfg(test)]
                tests::record("settings_show_again", again.response.rect);
                if again.inner {
                    show_again(state);
                }
            }
            None => {
                ui.label(RichText::new(t("wip.settings_showing")).size(small).color(theme.text_secondary()));
                let menu = ui.menu_button(t("wip.mute"), |ui| {
                    let mut chosen = None;
                    for c in choices() {
                        let pick = ui.button(t_dyn(&c.label)).hint_dyn(&c.hint);
                        #[cfg(test)]
                        tests::record(&format!("settings_{}", c.id), pick.rect);
                        if pick.clicked() {
                            chosen = Some(c);
                            ui.close_menu();
                        }
                    }
                    chosen
                });
                #[cfg(test)]
                tests::record("settings_mute", menu.response.rect);
                menu.response.hint("wip.mute_hint");
                if let Some(c) = menu.inner.flatten() {
                    choose(state, c, version, now);
                }
            }
        }
    });
    widgets::setting_hint(ui, theme, hint, t("wip.settings_hint"));
}

/// The bar's menu open, for the snapshots that photograph it (they have no cursor).
#[cfg(test)]
pub(crate) fn open_menu_for_snapshot(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.open_popup(egui::Id::new(MENU)));
}

#[cfg(test)]
#[path = "mute_tests.rs"]
pub(crate) mod tests;
