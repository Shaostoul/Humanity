// Child of gui/layers.rs (#[path]): the one layer order (navigation rung 3, docs/design/navigation.md
// section 6), read from egui itself, never assumed: the order egui draws its layers in
// (`Memory::layer_ids`, back to front) and what a press lands on (`Context::layer_id_at`). The frames
// run headless, as lib.rs runs them: the history, the top menu, the warning, the page, what floats,
// the toasts, then `arrange`.
//
// RED FIRST (2026-11-04). Against the app as rung 4 left it (the notices panel, its bell, the help
// button and the toasts in egui's Foreground, the incoming call a plain Middle window):
// - the_order_with_a_popup_the_notices_panel_and_an_urgent_card_open: "the notices panel is under
//   the popup's shade: notice_panel at 5, the shade at 1" (the panel drew over the dimmed screen and
//   took clicks through it).
// - a_popup_opened_while_the_notices_panel_is_open_covers_it: "round 0: the notices panel is under
//   the popup's shade: notice_panel at 5, the shade at 1".
// The three written after the fix were seen red in one run against layers.rs broken three ways on
// purpose (then put back): the primary menu's Areas in Foreground, `close_nearest` always false,
// and the shade's and the primary menu's website tokens swapped:
// - the_shade_covers_both_menus_and_the_warning: "a press on the bell lands on the popup's shade,
//   not Some(..), left: Some(false), right: Some(true)".
// - back_and_escape_close_every_popup_and_never_move_the_page_under_one: "Back with the image
//   viewer open leaves the page where it is, left: Chat, right: Files".
// - the_website_scale_is_the_same_order: "on the website PopupShade (z_menu_1 = 400) is nearer than
//   PrimaryDrop (z_menu_1_drop = 410)".
// - the_plan_settles_any_order_in_one_frame_or_two: passed throughout (it is the plan's own check).

use super::*;
use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

const W: f32 = 1600.0;
const H: f32 = 900.0;

struct App {
    ctx: egui::Context,
    theme: Theme,
    state: GuiState,
    t: f64,
    /// The warning's height on the last frame (read inside it: egui keeps it for its own pass).
    warning_h: f32,
}

/// What the test draws over the page this frame.
#[derive(Clone, Copy, Default)]
struct Open {
    notices: bool,
    popup: bool,
    call: bool,
    toast: bool,
}

impl App {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut state = GuiState::default();
        state.go(GuiPage::Files, crate::gui::How::Beside);
        App { ctx, theme, state, t: 1.0, warning_h: 0.0 }
    }

    /// One frame, in lib.rs's order.
    fn frame(&mut self, open: Open, events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let (theme, state, now) = (&self.theme, &mut self.state, self.t);
        state.notify.panel_open = open.notices;
        state.call_incoming = open.call.then(|| ("peer_key".to_string(), "Ada".to_string()));
        if open.toast && state.toasts.is_empty() {
            state.toast("Saved", crate::gui::ToastKind::Success, now);
        }
        let mut popup_open = open.popup;
        let mut warning_h = 0.0;
        let _ = self.ctx.run(input, |ctx| {
            crate::gui::history::frame(ctx, state);
            crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
            crate::gui::bottom_strip::draw(ctx, theme, state);
            warning_h = crate::gui::bottom_strip::height(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("The page");
            });
            if popup_open {
                crate::gui::widgets::dialog(ctx, theme, "layers_test_popup", "A popup", &mut popup_open, |ui| {
                    ui.allocate_space(egui::vec2(300.0, 120.0));
                });
            }
            crate::gui::pages::chat::draw_incoming_call_modal(ctx, theme, state);
            crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
            crate::gui::notice_panel::draw(ctx, theme, state);
            crate::gui::widgets::draw_toasts(ctx, theme, state);
            arrange(ctx);
        });
        self.warning_h = warning_h;
    }

    /// Back to front, as egui draws them.
    fn order(&self) -> Vec<LayerId> {
        self.ctx.memory(|m| m.layer_ids().collect())
    }

    /// The place of the layer whose id is `id` in the drawing order (back to front), by name.
    fn at(&self, id: Id, name: &str) -> usize {
        let order = self.order();
        order.iter().position(|l| l.id == id).unwrap_or_else(|| panic!("{name} is not drawn: {order:?}"))
    }

    fn rect(&self, id: Id, name: &str) -> egui::Rect {
        self.ctx.memory(|m| m.area_rect(id)).unwrap_or_else(|| panic!("{name} has no rect"))
    }

    /// What a press at `p` lands on.
    fn top(&self, p: egui::Pos2) -> Option<Id> {
        self.ctx.layer_id_at(p).map(|l| l.id)
    }
}

fn popup_window() -> Id {
    Id::new("layers_test_popup")
}
fn popup_shade() -> Id {
    Id::new("layers_test_popup").with("shade")
}
fn call() -> Id {
    Id::new("Incoming call")
}
fn notices() -> Id {
    Id::new("notice_panel")
}

/// The whole order at once: with a popup, the notices panel, an incoming call and a toast open,
/// egui draws (back to front) the notices panel, the popup's shade, its window, the call, the
/// toast: his 9, 5, 4, 2 and 1.
#[test]
fn the_order_with_a_popup_the_notices_panel_and_an_urgent_card_open() {
    let mut app = App::new();
    let all = Open { notices: true, popup: true, call: true, toast: true };
    for _ in 0..4 {
        app.frame(all, Vec::new());
    }
    let (n, s, w, c, t) = (
        app.at(notices(), "the notices panel"),
        app.at(popup_shade(), "the popup's shade"),
        app.at(popup_window(), "the popup's window"),
        app.at(call(), "the incoming call"),
        app.at(Id::new(("hos_toast", 0usize)), "the toast"),
    );
    assert!(n < s, "the notices panel is under the popup's shade: notice_panel at {n}, the shade at {s}");
    assert!(s < w, "the popup's window is on its shade: the shade at {s}, the window at {w}");
    assert!(w < c, "the incoming call is over the popup's window: Incoming call at {c}, the window at {w}");
    assert!(c < t, "a toast is over the incoming call: the toast at {t}, the call at {c}");
    // And what each order is, by the table: the urgent band in Foreground, the toast a tooltip.
    let order = app.order();
    let layer = |id: Id| order.iter().find(|l| l.id == id).copied().unwrap();
    assert_eq!(layer(call()).order, Layer::Urgent.order());
    assert_eq!(layer(Id::new(("hos_toast", 0usize))).order, Layer::Tooltip.order());
    assert_eq!(layer(notices()).order, Layer::PrimaryDrop.order());
    // A press on the call's card is the call's, over the popup; on the popup's middle, the window's.
    let card = app.rect(call(), "the incoming call");
    assert_eq!(app.top(card.center()), Some(call()), "the incoming call takes a press over the popup");
    let win = app.rect(popup_window(), "the popup's window");
    assert_eq!(app.top(win.center()), Some(popup_window()), "the popup's window takes a press on it");
    // The panel is shaded: a press on it lands on the shade.
    let panel = app.rect(notices(), "the notices panel");
    assert_eq!(app.top(panel.right_bottom() - egui::vec2(8.0, 8.0)), Some(popup_shade()), "a press on the notices panel lands on the popup's shade");
}

/// "This way it hides/shades the top menu buttons too": a press anywhere on both menus (the bell
/// and the help button included) and on the unfinished-software warning lands on the popup's shade.
#[test]
fn the_shade_covers_both_menus_and_the_warning() {
    let mut app = App::new();
    let open = Open { popup: true, ..Default::default() };
    for _ in 0..4 {
        app.frame(open, Vec::new());
    }
    let bell = app.rect(Id::new("notice_bell_button"), "the bell");
    let help = app.rect(Id::new("help_toggle_button"), "the help button");
    let rows = crate::gui::pages::escape_menu::rows_height(&app.theme);
    assert!(app.warning_h > 0.0, "the warning shows");
    let warning_y = H - app.warning_h / 2.0;
    for (what, p) in [
        ("the bell", bell.center()),
        ("the help button", help.center()),
        ("the primary row", egui::pos2(40.0, 12.0)),
        ("the secondary row", egui::pos2(W / 2.0, rows - 6.0)),
        ("the warning", egui::pos2(40.0, warning_y)),
        ("the page", egui::pos2(40.0, H / 2.0)),
    ] {
        assert_eq!(app.top(p).map(|i| i == popup_shade()), Some(true), "a press on {what} lands on the popup's shade, not {:?}", app.top(p));
    }
    // And they are drawn under it: the bell and the help button are Background, under every Middle layer.
    let order = app.order();
    for id in [Id::new("notice_bell_button"), Id::new("help_toggle_button")] {
        let l = order.iter().find(|l| l.id == id).unwrap();
        assert_eq!(l.order, Layer::Primary.order(), "{id:?} is part of the primary menu");
    }
}

/// Opened in the other order (the panel first, then a popup over it), and opened again: the
/// popup's shade is still over the panel.
#[test]
fn a_popup_opened_while_the_notices_panel_is_open_covers_it() {
    let mut app = App::new();
    for round in 0..2 {
        for _ in 0..3 {
            app.frame(Open { notices: true, ..Default::default() }, Vec::new());
        }
        // A press on the panel raises it in egui's list, the way a person's click does.
        let panel = app.rect(notices(), "the notices panel");
        let p = panel.right_bottom() - egui::vec2(8.0, 8.0);
        let m = egui::Modifiers::default();
        app.frame(Open { notices: true, ..Default::default() }, vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        app.frame(Open { notices: true, ..Default::default() }, vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        for _ in 0..3 {
            app.frame(Open { notices: true, popup: true, ..Default::default() }, Vec::new());
        }
        let (n, s) = (app.at(notices(), "the notices panel"), app.at(popup_shade(), "the popup's shade"));
        assert!(n < s, "round {round}: the notices panel is under the popup's shade: notice_panel at {n}, the shade at {s}");
        for _ in 0..2 {
            app.frame(Open::default(), Vec::new());
        }
    }
}

/// The website holds the same order: theme.ron's `z_*` token for each layer (`Layer::web_token`,
/// written into theme.css by gen-theme-css.js) is larger the nearer the layer is, and the numbers
/// are his, nearest first.
#[test]
fn the_website_scale_is_the_same_order() {
    let ron = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/gui/theme.ron")).expect("read theme.ron");
    let value = |token: &str| -> i32 {
        let line = ron.lines().find(|l| l.trim_start().starts_with(&format!("{token}:"))).unwrap_or_else(|| panic!("theme.ron has no {token}"));
        line.split(':').nth(1).unwrap().trim().trim_end_matches(',').parse().unwrap_or_else(|_| panic!("{token} is a whole number: {line}"))
    };
    for pair in Layer::ALL.windows(2) {
        let (near, far) = (pair[0], pair[1]);
        assert!(near.number() < far.number(), "his numbers count up from the viewer: {near:?} {} then {far:?} {}", near.number(), far.number());
        assert!(
            value(near.web_token()) > value(far.web_token()),
            "on the website {near:?} ({} = {}) is nearer than {far:?} ({} = {})",
            near.web_token(),
            value(near.web_token()),
            far.web_token(),
            value(far.web_token())
        );
    }
    // The theme's own defaults (a theme.ron without them) are the file's, and a theme saved from
    // Settings writes them back, so the scale is never lost.
    let without: String = ron.lines().filter(|l| !l.trim_start().starts_with("z_")).map(|l| format!("{l}\n")).collect();
    let theme: crate::gui::theme::Theme = ron::from_str(&without).expect("theme.ron without the scale still reads");
    assert_eq!((theme.z_shade, theme.z_menu_1, theme.z_page, theme.z_tooltip), (value("z_shade"), value("z_menu_1"), value("z_page"), value("z_tooltip")));
}

/// The plan itself, on made-up orders: it raises the fewest layers, and what egui raises anyway
/// is never left behind.
#[test]
fn the_plan_settles_any_order_in_one_frame_or_two() {
    let l = |n: &str| LayerId::new(Order::Middle, Id::new(n));
    let (page, drop, a, b) = (l("page"), l("drop"), l("popup_a"), l("popup_b"));
    let key = |x: LayerId| -> (u8, usize) {
        if x == page {
            (0, 0)
        } else if x == drop {
            (2, 0)
        } else if x == a {
            (3, 0)
        } else {
            (3, 1)
        }
    };
    // Already in order: nothing raised.
    assert_eq!(plan(&[page, drop, a, b], &[], key), (vec![], true));
    // The panel raised over the popups by a press: the two popups go back over it, in their order.
    assert_eq!(plan(&[page, a, b, drop], &[], key), (vec![a, b], true));
    // A page window newly shown over the panel and a popup: egui's one sort cannot put it under
    // both this frame (it raises the window too, and the window was added last), so the panel and
    // the popup are raised now and it settles on the next frame, which is asked for.
    assert_eq!(plan(&[drop, a, page], &[page], key), (vec![drop, a], false));
    // That next frame: nothing newly shown, the window on top; the two go back over it.
    assert_eq!(plan(&[drop, a, page], &[], key), (vec![drop, a], true));
    // A popup opened again keeps its old place in egui's list; egui raises it, and it alone.
    assert_eq!(plan(&[b, page, drop, a], &[b], key), (vec![b], true));
}

// ---- Back and Escape close every popup, through the one close path ---------------------------

/// One frame of a page with `draw` over it, the history first, as lib.rs runs them, then `arrange`.
fn popup_frame(app: &mut App, events: Vec<egui::Event>, mut draw: impl FnMut(&egui::Context, &Theme, &mut GuiState)) {
    app.t += 1.0 / 60.0;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))),
        time: Some(app.t),
        events,
        ..Default::default()
    };
    let (theme, state) = (&app.theme, &mut app.state);
    let _ = app.ctx.run(input, |ctx| {
        crate::gui::history::frame(ctx, state);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("The page");
        });
        draw(ctx, theme, state);
        arrange(ctx);
    });
}

/// The mouse's back button, pressed and let go over the middle of the window.
fn back_button() -> [Vec<egui::Event>; 2] {
    let pos = egui::pos2(W / 2.0, H / 2.0);
    let m = egui::Modifiers::default();
    let b = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Extra1, pressed, modifiers: m };
    [vec![egui::Event::PointerMoved(pos), b(true)], vec![b(false)]]
}

/// A popup drawn some other way than `widgets::dialog` (rung 4 left these out of Back and Escape):
/// how to open it, draw it, and see whether it is open.
struct Case {
    name: &'static str,
    open: fn(&mut GuiState),
    draw: fn(&egui::Context, &Theme, &mut GuiState),
    is_open: fn(&GuiState) -> bool,
    /// It asks for a decision: Back and Escape leave it, and the page, as they are.
    decide: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "the image viewer",
            open: |s| s.image_viewer_url = Some("https://example.invalid/picture.png".into()),
            draw: |c, t, s| crate::gui::widgets::image_cache_view::draw(c, t, s),
            is_open: |s| s.image_viewer_url.is_some(),
            decide: false,
        },
        Case {
            name: "the help modal",
            open: |s| {
                s.help_registry = crate::gui::widgets::help_modal::load_help_registry(&std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data"));
                s.active_help_topic = Some("identity".into());
            },
            draw: |c, t, s| crate::gui::widgets::help_modal::draw(c, t, &s.help_registry, &mut s.active_help_topic),
            is_open: |s| s.active_help_topic.is_some(),
            decide: false,
        },
        Case {
            name: "the passphrase change",
            open: |s| {
                s.passphrase_needed = true;
                s.passphrase_mode = crate::gui::PassphraseMode::Change;
            },
            draw: |c, t, s| crate::gui::widgets::passphrase_modal::draw(c, t, s),
            is_open: |s| s.passphrase_needed,
            decide: false,
        },
        Case {
            name: "the trading post",
            open: |s| s.vendor_open = true,
            draw: |c, t, s| crate::gui::pages::vendor::draw_vendor_modal(c, t, s),
            is_open: |s| s.vendor_open,
            decide: false,
        },
        Case {
            name: "the passphrase unlock",
            open: |s| {
                s.passphrase_needed = true;
                s.passphrase_mode = crate::gui::PassphraseMode::Unlock;
            },
            draw: |c, t, s| crate::gui::widgets::passphrase_modal::draw(c, t, s),
            is_open: |s| s.passphrase_needed,
            decide: true,
        },
        Case {
            name: "the privacy choice",
            open: |s| s.privacy_tier_prompt_open = true,
            draw: |c, t, s| crate::gui::pages::privacy::draw_privacy_tier_modal(c, t, s),
            is_open: |s| s.privacy_tier_prompt_open,
            decide: true,
        },
        Case {
            name: "the death screen",
            open: |s| s.player_death_cause = Some("a fall".into()),
            draw: |c, t, s| crate::gui::pages::hud::draw_death_screen(c, t, s),
            is_open: |s| s.player_death_cause.is_some(),
            decide: true,
        },
    ]
}

/// Back (the mouse's button) and Escape (lib.rs asks `history::close_popup`, as here) close each
/// popup first and the page stays; a popup asking for a decision stays, and so does the page; and
/// with it shut, Back goes back. Each popup's shade covers the page, its window on the shade.
#[test]
fn back_and_escape_close_every_popup_and_never_move_the_page_under_one() {
    for case in cases() {
        let mut app = App::new();
        app.state.go(GuiPage::Chat, crate::gui::How::Beside);
        app.state.go(GuiPage::Files, crate::gui::How::Beside);
        (case.open)(&mut app.state);
        let (draw, is_open) = (case.draw, case.is_open);
        let shown = move |c: &egui::Context, t: &Theme, s: &mut GuiState| {
            if is_open(s) {
                draw(c, t, s);
            }
        };
        for _ in 0..3 {
            popup_frame(&mut app, Vec::new(), shown);
        }
        let name = case.name;
        assert!(is_open(&app.state), "{name} opens");
        assert_eq!(open_popups(&app.ctx).len(), 1, "{name} is on the layer order as a popup");
        let (_, shade, window) = open_popups(&app.ctx)[0];
        assert_eq!(app.top(egui::pos2(6.0, H / 2.0)), Some(shade.id), "{name}'s shade covers the page");
        let window = window.unwrap_or_else(|| panic!("{name}'s window is seated on its shade"));
        let r = app.rect(window.id, name);
        assert_eq!(app.top(r.center()), Some(window.id), "{name}'s window is on its shade");
        for events in back_button() {
            popup_frame(&mut app, events, shown);
        }
        popup_frame(&mut app, Vec::new(), shown);
        assert_eq!(app.state.active_page, GuiPage::Files, "Back with {name} open leaves the page where it is");
        if case.decide {
            assert!(is_open(&app.state), "{name} asks for a decision: Back leaves it open");
            assert!(crate::gui::history::close_popup(&app.ctx), "Escape with {name} open does nothing more");
            popup_frame(&mut app, Vec::new(), shown);
            assert!(is_open(&app.state), "{name} asks for a decision: Escape leaves it open");
            continue;
        }
        assert!(!is_open(&app.state), "Back closes {name}");
        // Escape: the same close, asked the way lib.rs asks it.
        (case.open)(&mut app.state);
        for _ in 0..2 {
            popup_frame(&mut app, Vec::new(), shown);
        }
        assert!(crate::gui::history::close_popup(&app.ctx), "Escape finds {name} open");
        popup_frame(&mut app, Vec::new(), shown);
        assert!(!is_open(&app.state), "Escape closes {name}");
        popup_frame(&mut app, Vec::new(), shown);
        assert!(!crate::gui::history::close_popup(&app.ctx), "with {name} shut, Escape goes on to its other rules");
        for events in back_button() {
            popup_frame(&mut app, events, shown);
        }
        assert_eq!(app.state.active_page, GuiPage::Chat, "with {name} shut, Back goes back");
    }
}
