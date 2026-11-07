//! THE LAYERS: what is drawn over what, one order for the whole app (navigation rung 3,
//! 2026-11-04; docs/design/navigation.md section 6). The operator, 2026-11-02: "I guess when a full
//! screen popup happens. The click to exit background dark shading for the page under the popup is
//! like, layer 1, then the static popup window is layer 2, then any interactive bit are layer 3.
//! This way it hides/shades the top menu buttons too." And on the menus: "The primary top menu
//! would be on top of everything, we'll say layer 10. Then the second top menu is layer 20
//! underneath the primary. Then footer is 30. Then page content is 40+." An urgent alert (a call)
//! over an open popup: "Yes."
//!
//! THE ORDER, nearest the viewer first, in his numbers ([`Layer::number`]; section 6 keeps them and
//! numbers a popup's three parts so that lower is nearer there too, the window on its shade):
//!
//! | Layer | What |
//! |---|---|
//! | 1 | Tooltips, hints, toasts, the note while files are dragged over the window |
//! | 2 | Urgent: an incoming call and the call's own bar, an emergency alert |
//! | 3 | A full-screen popup's controls |
//! | 4 | The popup's window |
//! | 5 | The popup's shade: it dims and blocks everything farther away, both menus and the warning too |
//! | 9 | The primary row's drop-downs: the notices panel, the help panel |
//! | 10 | The primary menu, with its bell and help button |
//! | 19 | The secondary row's More |
//! | 20 | The secondary menu |
//! | 21 | The unfinished-software warning |
//! | 30 | The footer chat, and the chat videos' dock (an Area standing on the strip) |
//! | 40 | The page's drop-downs and its windows that are not full-screen popups (in the world: the HUD and its panels) |
//! | 41+ | The page |
//!
//! HOW EGUI HOLDS IT. egui draws its layers by `Order` (Background, Middle, Foreground, Tooltip),
//! and inside one order in a list of its own, where a layer moves to the top when it first shows
//! and when it is pressed. So, [`Layer::order`]:
//! - **10, 20, 21, 30 and 41+ are Background.** The menus, the warning and the page are panels,
//!   which never overlap (each takes its room), and the few parts of them drawn as Areas (the bell
//!   and the help button on the primary row, the chat's lock buttons, the world's crosshair) are
//!   Background Areas, drawn on their panel and under everything that floats.
//! - **40, 19, 9 and 5 with 4 are Middle**, put in that order at the end of every frame by
//!   [`arrange`]: the page's windows lowest, then the menus' drop-downs, then the full-screen
//!   popups, the one opened last nearest, each window glued just above its own shade
//!   (`set_sublayer`). egui's own raising (a layer that first shows, a press) is undone in the same
//!   frame where egui's one sort a frame allows it, and on the next frame otherwise.
//! - **3**: a popup's own buttons and boxes are drawn in its window; the drop-down lists in it are
//!   egui's (combo boxes, `popup_below_widget`), which egui draws in Foreground, over the window.
//!   egui's drop-downs elsewhere (a page's combo box, the menu's More, the warning's Mute) are in
//!   Foreground too and close at any click outside them; so a popup's shade, which takes every
//!   click, never has one open under it, and a popup that opens by itself closes any that is open
//!   ([`arrange`]).
//! - **2 is Foreground, raised to its top every frame** ([`arrange`]): over every popup, egui's
//!   drop-downs included. It never takes the popup's typing: an urgent card asks for no keyboard
//!   focus, and Back and Escape never close it; it is answered by its own buttons. Its band is at
//!   the top centre, under the menus ([`urgent_top`]).
//! - **1 is Tooltip**: nothing there takes a click.
//!
//! EVERY SURFACE PICKS ITS LAYER HERE: [`area`], [`window`] and [`painter`] build egui's Area,
//! Window and a painter on the layer named, and [`shade`] with [`seat`] make a full-screen popup.
//! `tests/layer_lint.rs` refuses `egui::Area::new`, `egui::Window::new` or an `egui::Order` written
//! anywhere else, so a new surface cannot skip the order (docs/design/ui-system.md, the layer rule).
//! The website holds the same order in its `--z-*` scale (theme.ron's `z_*` tokens,
//! [`Layer::web_token`]); a test holds the two to one order.
//!
//! ONE WAY TO CLOSE A POPUP. Every full-screen popup says how it may be closed ([`Close`]) when it
//! draws its shade. Back (the mouse's button, Alt+Left, the menu's arrow) and Escape ask
//! [`close_nearest`] first (through `history::close_popup`): it closes the popup opened last, or,
//! when that popup asks for a decision ([`Close::Decide`]: the passphrase at start, the privacy
//! choice, the death screen), does nothing more, so the page under it never moves.
//!
//! COST: a handful of layer ids gathered a frame and one sort of the few floating ones; kept
//! between frames only the open popups' ids, in egui's memory for the session. No file, no cache,
//! no repaint asked for unless an order took two frames to settle.

use std::collections::HashMap;

use egui::{Color32, Context, Id, LayerId, Order};

/// The kinds of surface, nearest the viewer first (the module's table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    /// 1: tooltips, hints and toasts. They never take a click.
    Tooltip,
    /// 2: urgent: an incoming call and the call's own bar, an emergency alert.
    Urgent,
    /// 3: a full-screen popup's controls (its drop-down lists are egui's, in Foreground).
    PopupControls,
    /// 4: the popup's window, seated on its shade with [`seat`].
    PopupWindow,
    /// 5: the popup's shade ([`shade`]).
    PopupShade,
    /// 9: the primary row's drop-downs: the notices panel, the help panel.
    PrimaryDrop,
    /// 10: the primary menu (a panel; its bell and help button are Areas on it).
    Primary,
    /// 19: the secondary row's More.
    SecondaryMore,
    /// 20: the secondary menu (a panel).
    Secondary,
    /// 21: the unfinished-software warning (a panel).
    Warning,
    /// 30: the footer chat (a panel, app-bottom-strip.md rung 2), and the chat videos' dock that follows
    /// the person round the app's pages (an Area, pages/chat/video_dock.rs; Background like the
    /// footer's panel, so the page's own windows, Middle, are drawn over it).
    Footer,
    /// 40: the page's drop-downs, context menus and windows that are not full-screen popups; in the
    /// world, the HUD, the chat panel, the creature editor and the talk card.
    PageFloat,
    /// 41 and up: the page; in the world, its tint and crosshair.
    Page,
}

impl Layer {
    /// Every layer, nearest the viewer first.
    pub const ALL: [Layer; 13] = [
        Layer::Tooltip,
        Layer::Urgent,
        Layer::PopupControls,
        Layer::PopupWindow,
        Layer::PopupShade,
        Layer::PrimaryDrop,
        Layer::Primary,
        Layer::SecondaryMore,
        Layer::Secondary,
        Layer::Warning,
        Layer::Footer,
        Layer::PageFloat,
        Layer::Page,
    ];

    /// The operator's number for it (section 6): lower is nearer the viewer.
    pub const fn number(self) -> u8 {
        match self {
            Layer::Tooltip => 1,
            Layer::Urgent => 2,
            Layer::PopupControls => 3,
            Layer::PopupWindow => 4,
            Layer::PopupShade => 5,
            Layer::PrimaryDrop => 9,
            Layer::Primary => 10,
            Layer::SecondaryMore => 19,
            Layer::Secondary => 20,
            Layer::Warning => 21,
            Layer::Footer => 30,
            Layer::PageFloat => 40,
            Layer::Page => 41,
        }
    }

    /// egui's order for it (the module's "How egui holds it").
    pub const fn order(self) -> Order {
        match self {
            Layer::Tooltip => Order::Tooltip,
            Layer::Urgent | Layer::PopupControls => Order::Foreground,
            Layer::PopupWindow | Layer::PopupShade | Layer::PrimaryDrop | Layer::SecondaryMore | Layer::PageFloat => Order::Middle,
            Layer::Primary | Layer::Secondary | Layer::Warning | Layer::Footer | Layer::Page => Order::Background,
        }
    }

    /// The website's custom property for it: theme.ron's token, which `scripts/gen-theme-css.js`
    /// writes into theme.css as `--z-...` (a larger z-index is nearer there).
    pub const fn web_token(self) -> &'static str {
        match self {
            Layer::Tooltip => "z_tooltip",
            Layer::Urgent => "z_urgent",
            Layer::PopupControls => "z_popup_controls",
            Layer::PopupWindow => "z_popup",
            Layer::PopupShade => "z_shade",
            Layer::PrimaryDrop => "z_menu_1_drop",
            Layer::Primary => "z_menu_1",
            Layer::SecondaryMore => "z_menu_2_drop",
            Layer::Secondary => "z_menu_2",
            Layer::Warning => "z_warning",
            Layer::Footer => "z_footer",
            Layer::PageFloat => "z_page_float",
            Layer::Page => "z_page",
        }
    }

    /// Where it stands among the Middle layers [`arrange`] orders: higher is nearer.
    fn middle_rank(self) -> u8 {
        match self {
            Layer::PopupShade | Layer::PopupWindow => 3,
            Layer::PrimaryDrop => 2,
            Layer::SecondaryMore => 1,
            _ => 0,
        }
    }
}

/// egui's Area on `layer`. Use this, never `egui::Area::new` (`tests/layer_lint.rs`).
pub fn area(ctx: &Context, id: impl Into<Id>, layer: Layer) -> egui::Area {
    let id = id.into();
    claim(ctx, layer, LayerId::new(layer.order(), id));
    egui::Area::new(id).order(layer.order())
}

/// egui's Window on `layer`, by its id (the id is what egui keeps its place and size under; a
/// window that used its title as its id passes `Id::new(title)`). Use this, never
/// `egui::Window::new` (`tests/layer_lint.rs`). A full-screen popup's window is
/// [`Layer::PopupWindow`], seated on its [`shade`] with [`seat`].
pub fn window<'open>(ctx: &Context, id: Id, title: impl Into<egui::WidgetText>, layer: Layer) -> egui::Window<'open> {
    claim(ctx, layer, LayerId::new(layer.order(), id));
    egui::Window::new(title).id(id).order(layer.order())
}

/// A painter on `layer` (paint only, it takes no click), under `id`.
pub fn painter(ctx: &Context, id: Id, layer: Layer) -> egui::Painter {
    ctx.layer_painter(LayerId::new(layer.order(), id))
}

/// The layer of the drop-down lists egui opens itself (combo boxes, `popup_below_widget`), for a
/// test or a check that asks whether one is showing.
pub const EGUI_DROP_DOWNS: Order = Order::Foreground;

/// The order tooltips are drawn in, for a check that asks whether a hint is showing.
pub const TOOLTIPS: Order = Order::Tooltip;

/// The top of the urgent band: just under both rows of the top menu on a page, near the top of the
/// window where there is no menu (the world, the title screen).
pub fn urgent_top(theme: &crate::gui::theme::Theme, page: crate::gui::GuiPage) -> f32 {
    if matches!(page, crate::gui::GuiPage::None | crate::gui::GuiPage::MainMenu) {
        8.0
    } else {
        crate::gui::pages::escape_menu::rows_height(theme) + 8.0
    }
}

// ---------------------------------------------------------------------------------------------
// Full-screen popups: the shade, the window seated on it, and the one way to close them.

/// How a full-screen popup may be closed, besides its own buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Close {
    /// A click on its shade, Back and Escape close it.
    Any,
    /// Back and Escape close it; a click on the shade does not (a form a stray click would lose).
    Keys,
    /// Only its own buttons: it asks for a decision. Back and Escape do nothing while it is the
    /// nearest popup, so the page under it stays where it is.
    Decide,
}

/// One open popup, in the order they opened.
#[derive(Debug, Clone, Copy)]
struct Open {
    id: Id,
    shade: LayerId,
    window: Option<LayerId>,
    close: Close,
    /// The pass it was last drawn in, and the pass it opened in.
    pass: u64,
    opened: u64,
}

const POPUPS: &str = "layers_popups";
const CLOSE_ASKED: &str = "layers_close_asked";
const CLAIMS: &str = "layers_claims";

fn popups(ctx: &Context) -> Vec<Open> {
    ctx.data(|d| d.get_temp::<Vec<Open>>(Id::new(POPUPS))).unwrap_or_default()
}

fn store_popups(ctx: &Context, list: Vec<Open>) {
    ctx.data_mut(|d| d.insert_temp(Id::new(POPUPS), list));
}

/// A popup's shade, drawn: where it is, and whether it was asked to close this frame.
#[derive(Debug, Clone, Copy)]
pub struct Shade {
    /// The shade's layer, for [`seat`].
    pub layer: LayerId,
    clicked: bool,
    asked: bool,
    close: Close,
}

impl Shade {
    /// Back or Escape asked the popup to close, or (when it may be closed that way) a click on its
    /// shade did. The popup closes; when Back or Escape asked, its shade was not drawn this frame.
    pub fn wants_close(&self) -> bool {
        self.asked || (self.clicked && self.close == Close::Any)
    }
}

/// The shade of the full-screen popup `popup` (layer 5): it dims everything farther away with
/// `dim` (both menus, the warning and the page) and takes every click on it. Drawn every frame the
/// popup is open, BEFORE its window; then [`seat`] the window on it. When Back or Escape asked this
/// popup to close, nothing is drawn and [`Shade::wants_close`] says so.
pub fn shade(ctx: &Context, popup: Id, close: Close, dim: Color32) -> Shade {
    let layer = LayerId::new(Layer::PopupShade.order(), popup.with("shade"));
    let asked = ctx.data_mut(|d| {
        if d.get_temp::<Id>(Id::new(CLOSE_ASKED)) == Some(popup) {
            d.remove::<Id>(Id::new(CLOSE_ASKED));
            true
        } else {
            false
        }
    });
    if asked {
        return Shade { layer, clicked: false, asked: true, close };
    }
    let now = ctx.cumulative_pass_nr();
    let mut list = popups(ctx);
    match list.iter().position(|p| p.id == popup) {
        // Drawn without a break since it opened: the same opening.
        Some(i) if list[i].pass + 1 >= now => {
            list[i].pass = now;
            list[i].close = close;
        }
        // Closed and opened again: a new opening, nearest of all.
        found => {
            if let Some(i) = found {
                list.remove(i);
            }
            list.push(Open { id: popup, shade: layer, window: None, close, pass: now, opened: now });
        }
    }
    store_popups(ctx, list);
    let screen = ctx.screen_rect();
    // What a press on the shade does, said where the pointer rests on it (the website's backdrops
    // say the same, `market.modal_backdrop_hint`).
    let hint = match close {
        Close::Any => "layers.shade_hint",
        Close::Keys => "layers.shade_keys_hint",
        Close::Decide => "layers.shade_decide_hint",
    };
    let resp = egui::Area::new(layer.id)
        .order(layer.order)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, dim);
            crate::gui::widgets::Hint::hint(ui.allocate_rect(screen, egui::Sense::click()), hint)
        });
    Shade { layer: resp.response.layer_id, clicked: resp.inner.clicked(), asked: false, close }
}

/// Seat the popup's window (layer 4) directly on its shade, this frame: egui then draws it just
/// above the shade wherever the shade stands, so the shade never tints its own window or swallows
/// its clicks (v0.849: "the tinted background claims the screen; the modal renders behind it").
pub fn seat(ctx: &Context, shade: &Shade, window: LayerId) {
    if shade.asked || window.order != shade.layer.order {
        return;
    }
    ctx.set_sublayer(shade.layer, window);
    let mut list = popups(ctx);
    if let Some(p) = list.iter_mut().find(|p| p.shade == shade.layer) {
        p.window = Some(window);
        store_popups(ctx, list);
    }
}

/// The open popups, nearest last: (its id, the shade's layer, the window's layer).
pub fn open_popups(ctx: &Context) -> Vec<(Id, LayerId, Option<LayerId>)> {
    let now = ctx.cumulative_pass_nr();
    popups(ctx).into_iter().filter(|p| p.pass + 1 >= now).map(|p| (p.id, p.shade, p.window)).collect()
}

/// Back or Escape: close the popup opened last. True when one is open, so the Back or Escape that
/// asked does nothing more; a popup asking for a decision ([`Close::Decide`]) stays as it is.
pub fn close_nearest(ctx: &Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    let Some(top) = popups(ctx).into_iter().rev().find(|p| p.pass + 1 >= now) else {
        return false;
    };
    if top.close != Close::Decide {
        ctx.data_mut(|d| d.insert_temp(Id::new(CLOSE_ASKED), top.id));
        ctx.request_repaint();
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The order, each frame.

/// Note that a surface on `layer` is drawn this frame as `id` (by [`area`] and [`window`]).
fn claim(ctx: &Context, layer: Layer, id: LayerId) {
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let inner = d.get_temp_mut_or_insert_with(Id::new(CLAIMS), || (now, Vec::<(Layer, LayerId)>::new()));
        if inner.0 != now {
            *inner = (now, Vec::new());
        }
        if !inner.1.iter().any(|(_, l)| *l == id) {
            inner.1.push((layer, id));
        }
    });
}

fn claims(ctx: &Context) -> Vec<(Layer, LayerId)> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, Vec<(Layer, LayerId)>)>(Id::new(CLAIMS))).filter(|a| a.0 == now).map(|a| a.1).unwrap_or_default()
}

/// Once a frame, after everything is drawn (lib.rs, the last thing in the frame): the urgent band
/// raised over every popup; a popup that opened by itself closes egui's drop-down under it; and
/// the floating Middle layers put in his order: the page's windows, then the menus' drop-downs,
/// then the popups, the one opened last nearest.
pub fn arrange(ctx: &Context) {
    let now = ctx.cumulative_pass_nr();
    let mut list = popups(ctx);
    let before = list.len();
    list.retain(|p| p.pass == now);
    if list.len() != before {
        store_popups(ctx, list.clone());
    }
    let claimed = claims(ctx);

    for (layer, id) in &claimed {
        if *layer == Layer::Urgent {
            ctx.move_to_top(*id);
        }
    }
    if list.iter().any(|p| p.opened == now) && ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
    }

    // Rank every Middle layer drawn this frame: a popup's shade by when it opened, the others by
    // their kind. Only layers drawn this frame are ever raised (a raised layer counts as shown).
    let mut rank: HashMap<LayerId, (u8, usize)> = HashMap::new();
    for (layer, id) in &claimed {
        if id.order == Order::Middle && !matches!(layer, Layer::PopupWindow) {
            rank.insert(*id, (layer.middle_rank(), 0));
        }
    }
    for (i, p) in list.iter().enumerate() {
        rank.insert(p.shade, (3, i));
    }
    let (current, mut forced) = ctx.memory(|m| {
        let a = m.areas();
        let current: Vec<LayerId> = m.layer_ids().filter(|l| rank.contains_key(l) && a.parent_layer(*l).is_none()).collect();
        // egui raises a layer the frame it first shows.
        let forced: Vec<LayerId> = current.iter().filter(|l| !a.visible_last_frame(l)).copied().collect();
        (current, forced)
    });
    // ... and the one under a press.
    if ctx.input(|i| i.pointer.any_pressed()) {
        if let Some(at) = ctx.input(|i| i.pointer.interact_pos()) {
            if let Some(l) = ctx.layer_id_at(at) {
                let top = ctx.memory(|m| m.areas().parent_layer(l)).unwrap_or(l);
                if rank.contains_key(&top) {
                    forced.push(top);
                }
            }
        }
    }
    let (raise, settled) = plan(&current, &forced, |l| rank.get(&l).copied().unwrap_or((0, 0)));
    for l in raise {
        ctx.move_to_top(l);
    }
    if !settled {
        ctx.request_repaint();
    }
}

/// Which of `current` (egui's order, back to front) to raise so that egui's one sort this frame
/// (the raised ones, in their order, after the rest) leaves them sorted by `key`, when `forced`
/// are raised by egui whatever we do; true when that settles it this frame. Otherwise everything
/// above the page's floats is raised now and the rest settles on the next frame.
fn plan(current: &[LayerId], forced: &[LayerId], key: impl Fn(LayerId) -> (u8, usize)) -> (Vec<LayerId>, bool) {
    let mut want = current.to_vec();
    want.sort_by_key(|l| key(*l));
    let pos: HashMap<LayerId, usize> = current.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    let in_order = |s: &[LayerId]| s.windows(2).all(|w| pos[&w[0]] < pos[&w[1]]);
    for k in (0..=want.len()).rev() {
        let (keep, raise) = want.split_at(k);
        if keep.iter().any(|l| forced.contains(l)) {
            continue;
        }
        if in_order(keep) && in_order(raise) {
            return (raise.to_vec(), true);
        }
    }
    (want.into_iter().filter(|l| key(*l).0 > 0).collect(), false)
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
