// Child of gui/layers.rs (#[path]): NOTHING ON THE TOOLTIP LAYER TAKES A CLICK (2026-11-15, the
// follow-up to BUG-316). Each surface drawn on layer 1 is drawn over a button on the page, and a
// hand on a mouse presses the button through it: the press must land on the button. egui 0.31
// gives every Area a widget rect the size of what it holds, `interactable(false)` included, and
// its hit test stops at the topmost layer with a widget over the pointer, so an Area there took
// the click (seven toasts buried the first-run privacy window's button, BUG-316). Each surface is
// now PAINTED (`widgets::painted_card`), and `tests/layer_lint.rs` refuses an Area or a Window on
// the tooltip layer.
//
// RED FIRST, 2026-11-15: run with every surface still the egui Area it was (the planet card and
// the chat's full timestamp already moved into the functions these tests call, unchanged
// otherwise): "0 passed; 6 failed", each "the press landed on the button under ...: false".
// What each test measures is the button's own `clicked()`, egui's answer to the press, never
// what the overlay was told.

use crate::gui::theme::Theme;
use crate::gui::GuiState;
use std::path::PathBuf;

/// The rig's window (2560x1387 at 100% scaling), as gui/pages/privacy_click_tests.rs uses.
const W: f32 = 2560.0;
const H: f32 = 1387.0;

struct Bench {
    ctx: egui::Context,
    theme: Theme,
    t: f64,
    /// Files dragged over the window, carried by every frame's input.
    hovered: Vec<PathBuf>,
}

impl Bench {
    fn new() -> Self {
        crate::config::keep_saves_off_disk();
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        Bench { ctx, theme, t: 1.0, hovered: Vec::new() }
    }

    /// One frame: the page (a button at `button`, when given), then `overlay` drawn over it, as
    /// the app draws the page and then its floating surfaces. Whether the button was clicked.
    fn frame(&mut self, events: Vec<egui::Event>, button: Option<egui::Rect>, overlay: &mut dyn FnMut(&egui::Context, &Theme)) -> bool {
        self.t += 1.0 / 30.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))),
            time: Some(self.t),
            events,
            hovered_files: self.hovered.iter().map(|p| egui::HoveredFile { path: Some(p.clone()), ..Default::default() }).collect(),
            ..Default::default()
        };
        let theme = &self.theme;
        let mut clicked = false;
        let _ = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
                if let Some(at) = button {
                    clicked = ui.put(at, egui::Button::new("under")).clicked();
                }
            });
            overlay(ctx, theme);
            crate::gui::layers::arrange(ctx);
        });
        clicked
    }

    /// Where the surface drawn under `id` stood on the last frame: a painted card's rect, else the
    /// egui Area's (the red run).
    fn rect_of(&self, id: egui::Id) -> Option<egui::Rect> {
        crate::gui::widgets::painted_card::rect(&self.ctx, id).or_else(|| self.ctx.memory(|m| m.area_rect(id)))
    }

    /// The surface `overlay` draws under `id`, with a button put under its middle and pressed
    /// there by a hand on a mouse (move, press, let go, one frame each, what the dev IPC's
    /// `click` sends). Where the surface stood, and whether the press landed on the button.
    fn press_under(&mut self, id: egui::Id, overlay: &mut dyn FnMut(&egui::Context, &Theme)) -> (egui::Rect, bool) {
        for _ in 0..3 {
            self.frame(Vec::new(), None, overlay);
        }
        let over = self.rect_of(id).expect("the surface is drawn");
        let button = egui::Rect::from_center_size(over.center(), egui::vec2(48.0, 18.0));
        for _ in 0..2 {
            self.frame(Vec::new(), Some(button), overlay);
        }
        let still = self.rect_of(id).expect("the surface is still drawn");
        assert!(still.contains_rect(button), "the precondition: the surface {still:?} covers the button {button:?}");
        let pos = button.center();
        let m = egui::Modifiers::default();
        let press = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: m };
        let mut clicked = self.frame(vec![egui::Event::PointerMoved(pos)], Some(button), overlay);
        clicked |= self.frame(vec![press(true)], Some(button), overlay);
        clicked |= self.frame(vec![press(false)], Some(button), overlay);
        clicked |= self.frame(Vec::new(), Some(button), overlay);
        (still, clicked)
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.5
}

/// THE F2/F3/F4 READOUTS take no click, and stay in the top-right corner (8 points in, 44 down).
#[test]
fn a_press_goes_through_the_diagnostics_readouts() {
    let mut b = Bench::new();
    let mut state = GuiState::default();
    state.show_perf_overlay = true;
    state.show_network_overlay = true;
    state.show_system_overlay = true;
    state.fps = 58.0;
    state.frame_times = vec![14.0, 17.5, 40.0, 16.0];
    let (rect, clicked) = b.press_under(egui::Id::new("diagnostics_overlay"), &mut |ctx, theme| crate::gui::pages::diagnostics::draw(ctx, theme, &state));
    assert!(near(rect.right(), W - 8.0) && near(rect.top(), 44.0), "the readouts stand in the top-right corner: {rect:?}");
    assert!(clicked, "the press landed on the button under the F2/F3/F4 readouts: {clicked}");
}

/// THE PLANET CARD over a targeted hologram pin takes no click, and stands centred 60 points
/// above the bottom.
#[test]
fn a_press_goes_through_the_planet_card() {
    let mut b = Bench::new();
    let (rect, clicked) = b.press_under(egui::Id::new("planet_info_tooltip"), &mut |ctx, theme| {
        crate::gui::pages::hud::planet_card::draw(ctx, theme, "Mars", "Iron oxide, water ice", "Uncolonized");
    });
    assert!(near(rect.center().x, W / 2.0) && near(rect.bottom(), H - 60.0), "the card stands centred above the bottom: {rect:?}");
    assert!(rect.width() > 100.0 && rect.height() > 60.0, "it shows its name, a line and two lines of words: {rect:?}");
    assert!(clicked, "the press landed on the button under the planet card: {clicked}");
}

/// THE CARD WHILE FILES ARE DRAGGED OVER THE WINDOW takes no click, and stands in the middle.
#[test]
fn a_press_goes_through_the_drop_overlay() {
    let mut b = Bench::new();
    b.hovered = vec![PathBuf::from("C:/somewhere/holiday.jpg"), PathBuf::from("C:/somewhere/notes.txt")];
    let mut state = GuiState::default();
    let (rect, clicked) = b.press_under(egui::Id::new("hos_drop_overlay_card"), &mut |ctx, theme| {
        crate::gui::drop_files::begin_frame(ctx);
        crate::gui::drop_files::finish(ctx, theme, &mut state);
    });
    assert!(near(rect.center().x, W / 2.0) && near(rect.center().y, H / 2.0), "the card stands in the middle: {rect:?}");
    assert!(clicked, "the press landed on the button under the drop overlay: {clicked}");
}

/// THE NOTE A DROP LEFT at the top of the window takes no click, and stands centred 48 points down.
#[test]
fn a_press_goes_through_the_drop_note() {
    let mut b = Bench::new();
    let mut said = false;
    let mut state = GuiState::default();
    let (rect, clicked) = b.press_under(egui::Id::new("hos_drop_note"), &mut |ctx, theme| {
        if !said {
            crate::gui::drop_files::say(ctx, "Nothing here takes files. Open the chat or Files and drop them there.");
            said = true;
        }
        crate::gui::drop_files::begin_frame(ctx);
        crate::gui::drop_files::finish(ctx, theme, &mut state);
    });
    assert!(near(rect.center().x, W / 2.0) && near(rect.top(), 48.0), "the note stands centred at the top: {rect:?}");
    assert!(clicked, "the press landed on the button under the drop note: {clicked}");
}

/// THE CHAT'S FULL TIMESTAMP beside a message's pull tab takes no click (a name or a link under
/// it stays pressable while the reaction popup is open), its right middle at the point given.
#[test]
fn a_press_goes_through_the_chat_full_timestamp() {
    let mut b = Bench::new();
    let at = egui::pos2(1200.0, 600.0);
    let id = egui::Id::new(("pill_ts_expand", 1_763_238_989_000_u64));
    let (rect, clicked) = b.press_under(id, &mut |ctx, theme| {
        crate::gui::pages::chat::rows::draw_full_timestamp(ctx, theme, id, at, "2026-11-15 20:36:29 UTC");
    });
    assert!(near(rect.right(), at.x) && near(rect.center().y, at.y), "its right middle is at the pull tab: {rect:?}");
    assert!(clicked, "the press landed on the button under the full timestamp: {clicked}");
}

/// THE TIP ABOVE A BAR (`Button::tooltip_above`, the chat composer's buttons) takes no click: the
/// pointer rests on a bar's button until its tip shows, then flicks onto a button under the tip
/// and presses in the same frame; the press lands on that button, not on the tip.
#[test]
fn a_press_goes_through_the_tip_above_a_bar() {
    let mut b = Bench::new();
    let bar_top = 1180.0;
    let a = egui::Rect::from_min_size(egui::pos2(1000.0, 1190.0), egui::vec2(60.0, 24.0));
    let tip_words = "Search this channel's messages for a word.";
    let mut under: Option<egui::Rect> = None;
    let mut clicked_under = false;
    // One frame of the page: the bar's button with its tip, and the button under the tip.
    let page = |b: &mut Bench, events: Vec<egui::Event>, under: Option<egui::Rect>| -> bool {
        b.t += 1.0 / 30.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))),
            time: Some(b.t),
            events,
            ..Default::default()
        };
        let theme = &b.theme;
        let mut clicked = false;
        let _ = b.ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
                if let Some(at) = under {
                    clicked = ui.put(at, egui::Button::new("under")).clicked();
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(a), |ui| {
                    crate::gui::widgets::Button::secondary("Find").tooltip_above(tip_words, bar_top).show(ui, theme);
                });
            });
            crate::gui::layers::arrange(ctx);
        });
        clicked
    };
    let tip = |b: &Bench| b.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(crate::gui::widgets::button::TIP_ABOVE_LAST)));
    page(&mut b, vec![egui::Event::PointerMoved(a.center())], None);
    for _ in 0..30 {
        page(&mut b, Vec::new(), under);
        if under.is_none() {
            under = tip(&b).map(|r| egui::Rect::from_center_size(r.center(), egui::vec2(48.0, 14.0)));
        }
    }
    let shown = tip(&b).expect("the tip shows once the pointer rests on the bar's button");
    let under = under.expect("a button under the tip");
    assert!(near(shown.bottom(), bar_top - 4.0) && near(shown.left(), a.left()), "the tip stands just above the bar at the button's left: {shown:?}");
    assert!(shown.contains_rect(under), "the precondition: the tip {shown:?} covers the button {under:?}");
    let pos = under.center();
    let m = egui::Modifiers::default();
    let press = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: m };
    clicked_under |= page(&mut b, vec![egui::Event::PointerMoved(pos), press(true)], Some(under));
    clicked_under |= page(&mut b, vec![press(false)], Some(under));
    clicked_under |= page(&mut b, Vec::new(), Some(under));
    assert!(clicked_under, "the press landed on the button under the tip above the bar: {clicked_under}");
}
