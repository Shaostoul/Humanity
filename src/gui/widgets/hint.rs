//! A plain-language hint on every control (the ELI5 mandate, docs/PRIORITIES.md
//! TIER 3, 2026-10-22): what this button, box or slider does, in words a person
//! new to computers understands, shown when the pointer rests on it.
//!
//! The words come from `data/i18n/en.json` by KEY (a hint is words like any
//! other, so it is translated like any other), and the person's hint mode
//! (Settings > Appearance, `HintDisplay`) decides whether they show: Full and
//! Hover show them, Off hides them for someone who knows the app. The website's
//! twin is `data-i18n-tip` / `data-tip-desc` read by `web/shared/shell.js`.
//!
//! Three ways to give a control its hint:
//!
//! ```ignore
//! use crate::gui::widgets::hint::Hint;
//! ui.button(t("notes.new")).hint("notes.new_hint");                 // any Response
//! Button::primary(t("notes.save")).hint("notes.save_hint").show(ui, theme);
//! with_hint(ui, "settings.dark_mode_hint", |ui| widgets::toggle(ui, theme, label, &mut v));
//! ```
//!
//! `with_hint` is for the project's helpers that return `bool` rather than a
//! `Response` (`toggle`, `labeled_slider`, `custom_checkbox`, `search_bar`,
//! `compact_button`, `stepper_button`, the `*_button` functions): the helper
//! takes the waiting hint and puts it on the control it draws, and on its
//! label too. `tests/i18n_lint.rs` counts every control without a hint, page by
//! page, and that count only goes down.

use egui::{Id, Response, Ui};

use crate::gui::HintDisplay;

const MODE_ID: &str = "hos_hint_display";
const PENDING_ID: &str = "hos_pending_hint";

/// Set this frame's hint mode on a context (the page dispatch does it once per
/// frame from `state.settings.hint_display`). A context never told shows hints.
pub fn set_mode(ctx: &egui::Context, mode: HintDisplay) {
    ctx.data_mut(|d| d.insert_temp(Id::new(MODE_ID), mode));
}

/// The hint mode in force on this context.
pub fn mode(ctx: &egui::Context) -> HintDisplay {
    ctx.data(|d| d.get_temp::<HintDisplay>(Id::new(MODE_ID))).unwrap_or_default()
}

/// Are hints shown on this context?
pub fn shown(ctx: &egui::Context) -> bool {
    !matches!(mode(ctx), HintDisplay::Off)
}

/// Hints on a `Response`.
pub trait Hint {
    /// Show the words of `key` (from `data/i18n/`) when the pointer rests on
    /// this control, also while it is disabled (where saying what it would do,
    /// and why it cannot yet, matters most).
    fn hint(self, key: &'static str) -> Response;
    /// `hint` for a key built at run time (one per item in a list).
    fn hint_dyn(self, key: &str) -> Response;
}

impl Hint for Response {
    fn hint(self, key: &'static str) -> Response {
        show_words(self, crate::i18n::t(key))
    }
    fn hint_dyn(self, key: &str) -> Response {
        show_words(self, crate::i18n::t_dyn(key))
    }
}

/// Put words on a response as its hover hint, unless hints are off.
pub(crate) fn show_words(resp: Response, words: &str) -> Response {
    if words.is_empty() || !shown(&resp.ctx) {
        return resp;
    }
    let resp = resp.on_hover_text(words).on_disabled_hover_text(words);
    show_on_focus(&resp, words);
    resp
}

/// The hint of a control the KEYBOARD has reached (WCAG 1.4.13 and 2.1.1; the
/// accessibility audit, docs/design/wcag-audit-2026-10-23.md): a person who moves
/// with Tab gets the same plain words a mouse user gets by resting the pointer.
/// egui gives a button, a box or a slider focus only through the keyboard (a click
/// does not focus it), so focus means Tab. Not on a text box, so nothing sits under
/// the line someone is typing (a text box keeps an edit state; egui's own
/// `wants_keyboard_input` is true for ANY focused widget, so it cannot tell them apart).
pub(crate) fn show_on_focus(resp: &Response, words: &str) {
    if words.is_empty() || !resp.has_focus() || resp.hovered() {
        return;
    }
    if egui::widgets::text_edit::TextEditState::load(&resp.ctx, resp.id).is_some() {
        return;
    }
    resp.show_tooltip_text(words);
}

const NAME_ID: &str = "hos_pending_control_name";

/// Draw `f` with a name for the control inside it: the words of the row it sits in
/// (`settings_row`), so a box or a slider that paints itself can tell a screen reader
/// what it is (WCAG 4.1.2). Like `with_hint`, it lasts the scope of `f`.
pub fn with_name<R>(ui: &mut Ui, name: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
    let before = pending_name(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(NAME_ID), Some(name.to_string())));
    let r = f(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(NAME_ID), before));
    r
}

/// Name and hint the keyboard stops an egui `ScrollArea` makes for itself (WCAG 4.1.2
/// and 2.1.1). Once its content is larger than the area, egui makes the scroll bar a
/// Tab stop, and with drag-to-scroll the content too, with no name and no hint, so a
/// keyboard user lands on silence (found 2026-10-30, when the Tasks page's Todo
/// column first overflowed in the a11y walk). Call it after `show`, with the output's
/// `id`; the ids are egui 0.31's own (`scroll_area.rs`: `id.with("area")` for the
/// content, `id.with(d)` for each bar). Nothing happens while the area does not scroll.
pub fn name_scroll_area(ui: &Ui, area: Id, name: &str, hint_key: &'static str) {
    let words = crate::i18n::t(hint_key);
    for id in [area.with("area"), area.with(0usize), area.with(1usize)] {
        if let Some(resp) = ui.ctx().read_response(id) {
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, name));
            if shown(ui.ctx()) {
                show_on_focus(&resp, words);
            }
        }
    }
}

/// The name waiting for the control being drawn, or "" when there is none.
pub(crate) fn pending_name(ui: &Ui) -> Option<String> {
    ui.ctx().data(|d| d.get_temp::<Option<String>>(Id::new(NAME_ID))).flatten()
}

/// Draw `f` with a hint for the project controls inside it (see the module
/// doc): a helper such as `toggle` puts it on its box AND its label, and
/// `labeled_slider_entry` on its track and its number box, because they are one
/// control to the person. Wrap ONE control; the hint is gone after `f`.
pub fn with_hint<R>(ui: &mut Ui, key: &'static str, f: impl FnOnce(&mut Ui) -> R) -> R {
    let before = pending(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(PENDING_ID), Some(key)));
    let r = f(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(PENDING_ID), before));
    r
}

/// Draw `f` with no hint waiting: the inside of a control that took the waiting
/// hint for itself (a collapsible section's header) is not that control.
pub(crate) fn without_pending<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    let before = pending(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(PENDING_ID), None::<&'static str>));
    let r = f(ui);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(PENDING_ID), before));
    r
}

/// The hint waiting for the controls being drawn, if any.
pub(crate) fn pending(ui: &Ui) -> Option<&'static str> {
    ui.ctx().data(|d| d.get_temp::<Option<&'static str>>(Id::new(PENDING_ID))).flatten()
}

/// Give a freshly drawn part of a control the waiting hint, if there is one.
pub(crate) fn apply_pending(ui: &Ui, resp: Response) -> Response {
    match pending(ui) {
        Some(key) => resp.hint(key),
        None => resp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, pos: egui::Pos2, f: &mut dyn FnMut(&mut Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0))),
            events: vec![egui::Event::PointerMoved(pos)],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f(ui));
        });
    }

    /// Is any tooltip layer showing on this context?
    fn tooltip_up(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Tooltip))
    }

    /// Rest the pointer on a hinted button and see whether a tooltip comes up,
    /// in each hint mode. The words are the shipped English for the key.
    fn hover_shows_tip(mode: HintDisplay) -> bool {
        let ctx = egui::Context::default();
        ctx.style_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.show_tooltips_only_when_still = false;
        });
        let rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let mut draw = |ui: &mut Ui| {
            set_mode(ui.ctx(), mode);
            rect.set(ui.button("X").hint("settings.language_hint").rect);
        };
        frame(&ctx, egui::pos2(-10.0, -10.0), &mut draw);
        let at = rect.get().center();
        for _ in 0..3 {
            frame(&ctx, at, &mut draw);
        }
        tooltip_up(&ctx)
    }

    /// A person who moves with Tab gets the hint too (WCAG 1.4.13, the accessibility
    /// audit): Tab onto a hinted button with the pointer nowhere near it, and the hint
    /// comes up; with hints Off it does not.
    #[test]
    fn a_hint_shows_on_keyboard_focus_unless_hints_are_off() {
        for (mode, want) in [(HintDisplay::Full, true), (HintDisplay::Hover, true), (HintDisplay::Off, false)] {
            let ctx = egui::Context::default();
            ctx.style_mut(|s| {
                s.interaction.tooltip_delay = 0.0;
                s.interaction.show_tooltips_only_when_still = false;
            });
            let run = |events: Vec<egui::Event>| {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0))),
                    events,
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        set_mode(ui.ctx(), mode);
                        ui.button("X").hint("settings.language_hint");
                    });
                });
            };
            let tab = |pressed| egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            run(Vec::new());
            run(vec![tab(true), tab(false)]);
            run(Vec::new());
            run(Vec::new());
            assert!(ctx.memory(|m| m.focused()).is_some(), "Tab reached the button ({mode:?})");
            assert_eq!(tooltip_up(&ctx), want, "the hint on keyboard focus ({mode:?})");
        }
    }

    #[test]
    fn a_hint_shows_on_hover_unless_hints_are_off() {
        assert!(hover_shows_tip(HintDisplay::Full), "Full shows the hint");
        assert!(hover_shows_tip(HintDisplay::Hover), "Hover shows the hint");
        assert!(!hover_shows_tip(HintDisplay::Off), "Off hides it");
    }

    #[test]
    fn a_waiting_hint_lasts_its_scope_and_no_longer() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(pending(ui), None);
                with_hint(ui, "settings.language_hint", |ui| {
                    assert_eq!(pending(ui), Some("settings.language_hint"));
                    with_hint(ui, "nav.home", |ui| assert_eq!(pending(ui), Some("nav.home")));
                    assert_eq!(pending(ui), Some("settings.language_hint"), "an inner scope gives the outer one back");
                });
                assert_eq!(pending(ui), None, "the hint does not leak past its scope");
            });
        });
    }
}
