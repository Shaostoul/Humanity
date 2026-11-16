// Child of gui/pages/privacy.rs (#[path]): the first-run "Choose your privacy" window answered by
// a click, as a person answers it, over the first-person world as engine/frame_ui_run.rs draws it
// (BUG-316, 2026-11-15).
//
// RED FIRST, 2026-11-15. Run against the app's behaviour before the fix (toasts drawn as egui
// Areas, the same words stacked again, the world's notices let out under a decision popup, all
// three put back on purpose, this file unchanged): "1 passed; 3 failed".
// - a_click_closes_the_first_run_window_with_the_garden_notices_over_it: "with the garden's
//   notices over the window: the click closed the window" (its precondition, a toast over the
//   button, held): the press landed on the top toast (an Area's widget rect; egui's hit test
//   stops at the topmost layer with a widget over the pointer), as verify-copresence's fresh
//   sandbox showed that day ("the press landed on {layer: Tooltip ..., rect_px: [916.9, 879.5,
//   1660.1, 989.5]}", seven garden notices piled over the window in its view_fail.png).
// - the_same_notice_is_never_stacked_twice: "the same words are shown once" (seven, the cap).
// - the_worlds_notices_wait_while_the_privacy_window_asks: "no world notice over the question:
//   ["The sunflower plants in oilbed 5 are flowering, ...".
// The first test, a click with nothing over the window, passed throughout: the window and its
// button were never the fault.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::GuiPage;

/// The rig's window (2560x1387 at 100% scaling, the screenshot of run 20261115-203629).
const W: f32 = 2560.0;
const H: f32 = 1387.0;
const APPLY: &str = "Use this privacy level";

/// The world's notices a brand-new identity's homestead gave on entering, each TWICE (the home's
/// world, then the server's), as verify-copresence's view_fail.png of 2026-11-15 shows them.
fn garden_notices() -> Vec<String> {
    let sunflower = |bed: u32| {
        format!(
            "The sunflower plants in oilbed {bed} are flowering, and indoors nothing carries their pollen. Left alone, \
             sunflower sets about 84% of a full crop. Hand-pollinate them from the Garden panel while they flower \
             (sunflower: rub the ring of open florets on each head with a soft glove or cloth, and move between heads, \
             every 2 garden days), or keep a bumblebee colony in a hive near them."
        )
    };
    let strawberry = "The strawberry plants in ntower 0 are flowering, and indoors nothing carries their pollen. Left alone, \
        strawberry sets about 60% of a full crop. Hand-pollinate them from the Garden panel while they flower (strawberry: \
        brush the centre of each open flower with a soft paintbrush every day, flower to flower, every 1 garden day), or keep \
        a bumblebee colony in a hive near them, or run a circulating fan over them (strawberry sets about 90% of a full crop \
        under one)."
        .to_string();
    let once = vec![strawberry, sunflower(2), sunflower(5), sunflower(8)];
    once.iter().chain(once.iter()).cloned().collect()
}

struct World {
    ctx: egui::Context,
    theme: Theme,
    state: GuiState,
    t: f64,
}

impl World {
    /// A brand-new identity standing in the world: connected and identified, no level ever chosen,
    /// the world entered and running.
    fn new() -> Self {
        crate::config::keep_saves_off_disk();
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut state = GuiState::default();
        state.active_page = GuiPage::None;
        state.show_hud = true;
        state.ws_identified = true;
        state.cursor_free = true;
        state.world_entered = true;
        assert!(state.settings.privacy_tier.is_empty(), "a new identity has chosen no level");
        World { ctx, theme, state, t: 1.0 }
    }

    /// One frame as the engine runs it: the world's frame note before the systems tick
    /// (`note_world_frame`), then the world's UI in engine/frame_ui_run.rs's order (every surface
    /// there that needs only the GUI state), `layers::arrange`, and the decision flag it sets.
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.t += 1.0 / 30.0;
        self.state.note_world_frame(true);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let (theme, state) = (&self.theme, &mut self.state);
        self.ctx.run(input, |ctx| {
            crate::gui::history::frame(ctx, state);
            crate::gui::bottom_strip::draw(ctx, theme, state);
            if state.active_page == GuiPage::None && state.show_hud {
                crate::gui::pages::hud::draw(ctx, theme, state, 0.0, glam::Mat4::IDENTITY, glam::Vec3::ZERO);
                crate::gui::pages::hud::draw_machine_recipe_selector(ctx, theme, state);
            }
            first_connect_prompt(ctx, theme, state);
            crate::gui::pages::safety_protected::draw_pin_prompt(ctx, theme, state);
            if crate::gui::pages::hud::crosshair_visible(state) {
                let painter = crate::gui::layers::painter(ctx, egui::Id::new("crosshair"), crate::gui::layers::Layer::Page);
                painter.circle_filled(ctx.screen_rect().center(), 3.0, theme.info());
            }
            crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
            crate::gui::pages::diagnostics::draw(ctx, theme, state);
            let _ = crate::gui::pages::weather_panel::draw(ctx, theme, state);
            let _ = crate::gui::pages::cloud_dev::draw(ctx, theme, state);
            crate::gui::map_credit::draw(ctx, theme, state);
            crate::gui::widgets::draw_toasts(ctx, theme, state);
            crate::gui::first_minutes::draw(ctx, theme, state);
            crate::gui::widgets::draw_underwater_tint(ctx, state);
            crate::gui::layers::arrange(ctx);
            state.decision_open = crate::gui::layers::deciding(ctx);
        })
    }

    /// A few frames with no input; the last one's output.
    fn settle(&mut self, frames: usize) -> egui::FullOutput {
        let mut out = self.frame(Vec::new());
        for _ in 1..frames {
            out = self.frame(Vec::new());
        }
        out
    }

    /// Where the button is drawn in `out`, if it is.
    fn apply_button(out: &egui::FullOutput) -> Option<egui::Rect> {
        find_text_in_shapes(&out.shapes, APPLY).filter(|f| f.text == APPLY).map(|f| f.rect)
    }

    /// A hand on a mouse: move onto `pos`, press, let go, one frame each (what the dev IPC's
    /// `click` sends, engine/ipc.rs `poll_ui_request`); then a few frames. The last one's output.
    fn click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
        let m = egui::Modifiers::default();
        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: m };
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.settle(4)
    }

    /// The window answered: the level applied and kept, the window not drawn.
    fn assert_answered(&self, after: &egui::FullOutput, what: &str) {
        assert!(!self.state.privacy_tier_prompt_open, "{what}: the click closed the window");
        assert_eq!(self.state.settings.privacy_tier, "private", "{what}: the click applied the level picked (Private, the default)");
        assert!(World::apply_button(after).is_none(), "{what}: the window is not drawn on the frames after the click");
    }
}

/// A PERSON'S CLICK ANSWERS THE FIRST-RUN PRIVACY WINDOW. The window, drawn over the world for a
/// brand-new identity, is answered by a pointer press and release on "Use this privacy level"
/// through egui's own input, and is gone on the frames after: the level applied and kept, so it
/// never asks again.
#[test]
fn a_click_on_use_this_privacy_level_closes_the_first_run_window_over_the_world() {
    let mut w = World::new();
    let out = w.settle(5);
    assert!(w.state.privacy_tier_prompt_open, "a new identity in the world is asked");
    let rect = World::apply_button(&out).expect("the window draws its button");
    let after = w.click(rect.center());
    w.assert_answered(&after, "nothing over the window");
}

/// TOASTS TAKE NO CLICK (BUG-316). The garden's notices already on screen when the window is
/// answered, stacked up from the bottom over the window and its button (the rig's view_fail.png),
/// must let the click through to the button under them: a toast is a tooltip of the whole app,
/// over everything, no click (gui/layers.rs layer 1).
#[test]
fn a_click_closes_the_first_run_window_with_the_garden_notices_over_it() {
    let mut w = World::new();
    w.settle(3);
    // Already toasts (shown before the window asked, so not held), the newest highest.
    let now = w.ctx.input(|i| i.time);
    for n in garden_notices() {
        w.state.notice(n, now);
    }
    let out = w.settle(3);
    let rect = World::apply_button(&out).expect("the window draws its button");
    // The precondition, so this cannot pass by the toasts standing elsewhere: a notice's text sits
    // over the button, its frame round it.
    let notices: Vec<egui::Rect> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) if t.galley.text().contains("nothing carries their pollen") => Some(t.galley.rect.translate(t.pos.to_vec2())),
            _ => None,
        })
        .collect();
    assert!(
        notices.iter().any(|n| n.expand2(egui::vec2(40.0, 12.0)).intersects(rect)),
        "a toast stands over the button: toasts {notices:?}, button {rect:?}"
    );
    let after = w.click(rect.center());
    w.assert_answered(&after, "with the garden's notices over the window");
}

/// THE SAME NOTICE IS NEVER STACKED TWICE (BUG-316): told each bed twice, the stack holds each
/// once, the one told again on top.
#[test]
fn the_same_notice_is_never_stacked_twice() {
    let mut w = World::new();
    let now = 1.0;
    for n in garden_notices() {
        w.state.notice(n, now);
    }
    assert_eq!(w.state.toasts.len(), 4, "the same words are shown once");
    let texts: Vec<&str> = w.state.toasts.iter().map(|t| t.text.as_str()).collect();
    assert!(texts[3].contains("oilbed 8") && texts[0].starts_with("The strawberry"), "in the order last told: {texts:?}");
    // A toast told again while it shows comes back to the top with a fresh life.
    w.state.toast("Saved", crate::gui::ToastKind::Success, 5.0);
    w.state.toast("Saved", crate::gui::ToastKind::Success, 6.0);
    assert_eq!(w.state.toasts.iter().filter(|t| t.text == "Saved").count(), 1);
    assert_eq!(w.state.toasts.last().map(|t| t.created), Some(6.0));
}

/// THE WORLD'S NOTICES WAIT WHILE THE PRIVACY WINDOW ASKS (BUG-316). Entering the world let the
/// garden's held notices out on the frame the window was asking: now they wait for the answer, so
/// nothing stands over the question, and are shown on the frame after it. The app's own notices
/// (a reply to what the person did) are not held.
#[test]
fn the_worlds_notices_wait_while_the_privacy_window_asks() {
    let mut w = World::new();
    w.settle(3);
    assert!(w.state.decision_open, "the privacy window asks for a decision");
    for n in garden_notices() {
        w.state.world_notice(n);
    }
    w.state.reply_notice("Nothing to plant: pick a seed first.".to_string());
    let out = w.settle(3);
    assert!(
        !w.state.toasts.iter().any(|t| t.text.contains("nothing carries their pollen")),
        "no world notice over the question: {:?}",
        w.state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    assert!(w.state.toasts.iter().any(|t| t.text.starts_with("Nothing to plant")), "a reply is shown at once");
    let rect = World::apply_button(&out).expect("the window draws its button");
    let after = w.click(rect.center());
    w.assert_answered(&after, "with the world's notices waiting");
    assert!(!w.state.decision_open, "nothing asks any more");
    let garden = w.state.toasts.iter().filter(|t| t.text.contains("nothing carries their pollen")).count();
    assert_eq!(garden, 4, "once answered, the world's notices are shown, each once");
}
