//! A THROWAWAY mock-up of the double top menu, for docs/design/navigation.md (2026-11-02).
//!
//! It draws the primary and secondary menus, a page in one and in three columns, the footer chat
//! folded and unfolded, the unfinished-software warning (the real one, `bottom_strip`) and a
//! full-screen popup with an urgent call over it, each band tagged with its layer number in the
//! operator's numbering (lower is nearer the viewer). It is test-only, draws nothing in the app,
//! and is the design's evidence: delete this file when the design's rung 2 (the real double menu)
//! lands with its own snapshots.
//!
//! The product words (the menus' names, the six questions) are keys in data/i18n/en.json, the
//! words the real build will use. The layer tags, the sample messages, numbers and file names are
//! the picture's annotations and stay literals here (a test file, which i18n_lint does not scan).
//!
//! Render: `just snapshot nav_double_tv` (and _tv_hover, _tv_popup, _laptop, _phone); the five
//! #[test] wrappers are at the end of ui_snapshots.rs, where the snapshot tooling finds them.
//! Pictures: tests/snapshots/nav_double_*.png

use egui::{pos2, vec2, Color32, CornerRadius, FontId, Frame, Id, LayerId, Order, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use humanity_core::i18n::t;

use crate::gui::theme::Theme;
use crate::gui::ui_snapshots::render_page_png;
use crate::gui::widgets::icons;
use crate::gui::{GuiPage, GuiState};

/// One primary category: its name (a key), the page its icon is drawn for, its colour and its
/// secondary items (a key each, and whether the page is new in the design).
struct Cat {
    name: &'static str,
    icon: GuiPage,
    color: fn(&Theme) -> Color32,
    items: &'static [(&'static str, bool)],
}

/// The design's eight categories, in order (docs/design/navigation.md section 3).
const CATS: [Cat; 8] = [
    Cat {
        name: "nav.humanity",
        icon: GuiPage::Humanity,
        color: Theme::nav_reality,
        items: &[
            ("nav.mission", false),
            ("nav.people", true),
            ("nav.governance", false),
            ("nav.laws", false),
            ("nav.rules", true),
            ("nav.donate", false),
            ("nav.roadmap", true),
            ("nav.devlog", true),
        ],
    },
    Cat {
        name: "nav.play",
        icon: GuiPage::None,
        color: Theme::nav_sim,
        items: &[
            ("nav.characters", false),
            ("nav.worlds", true),
            ("nav.servers", true),
            ("nav.quests", false),
            ("nav.inventory", false),
            ("nav.crafting", false),
        ],
    },
    Cat {
        name: "nav.chat",
        icon: GuiPage::Chat,
        color: Theme::section_cyan,
        items: &[("nav.watch", false), ("nav.studio", false), ("nav.files", false), ("nav.guilds", false)],
    },
    Cat {
        name: "nav.home",
        icon: GuiPage::Homes,
        color: Theme::nav_legacy_green,
        items: &[
            ("nav.profile", false),
            ("nav.tasks", false),
            ("nav.maps", false),
            ("nav.wallet", false),
            ("nav.market", false),
            ("nav.trade", false),
        ],
    },
    Cat {
        name: "nav.apps",
        icon: GuiPage::Apps,
        color: Theme::nav_tools,
        items: &[("nav.notes", false), ("nav.calendar", false), ("nav.calculator", false), ("nav.browser", false)],
    },
    Cat {
        name: "nav.library",
        icon: GuiPage::Library,
        color: Theme::section_magenta,
        items: &[("nav.how_to", false)],
    },
    Cat {
        name: "nav.dev",
        icon: GuiPage::Dev,
        color: Theme::nav_dev,
        items: &[
            ("nav.bugs", false),
            ("nav.testing", false),
            ("nav.performance", false),
            ("nav.planet_tuner", false),
            ("nav.claude", false),
            ("nav.journal", false),
        ],
    },
    Cat {
        name: "nav.settings",
        icon: GuiPage::Settings,
        color: Theme::nav_settings,
        items: &[("nav.server_settings", false)],
    },
];

#[derive(Clone, Copy, PartialEq)]
enum Page {
    /// The Humanity overview, one column.
    Overview,
    /// The Files page, three columns.
    FilesThree,
}

#[derive(Clone, Copy, PartialEq)]
enum Footer {
    Folded,
    Unfolded,
    /// The website has no chat footer (desktop only for now).
    Off,
}

#[derive(Clone, Copy)]
struct Mock {
    phone: bool,
    /// The category of the page on screen.
    active: usize,
    /// The secondary item on screen, when the page is not the category's own.
    active_item: Option<usize>,
    /// A category the pointer rests on, previewing its pages in the secondary row.
    preview: Option<usize>,
    page: Page,
    footer: Footer,
    popup: bool,
}

// ── Sizes the design proposes as theme tokens (section 7) ───────────────────────────────────────
const PRIMARY_H: f32 = 48.0;
const PRIMARY_BTN_H: f32 = 36.0;
const SECONDARY_H: f32 = 40.0;
const SECONDARY_BTN_H: f32 = 30.0;
const SECONDARY_FONT: f32 = 14.5;
const PHONE_PRIMARY_H: f32 = 60.0;
const PHONE_SECONDARY_H: f32 = 48.0;
const ICON: f32 = 18.0;

fn tint(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Words laid out to `wrap` wide at `pos`; returns their height.
fn words(p: &Painter, pos: Pos2, s: &str, size: f32, color: Color32, wrap: f32) -> f32 {
    let g = p.layout(s.to_string(), FontId::proportional(size), color, wrap);
    let h = g.size().y;
    p.galley(pos, g, color);
    h
}

fn width_of(p: &Painter, s: &str, size: f32) -> f32 {
    p.layout_no_wrap(s.to_string(), FontId::proportional(size), Color32::WHITE).size().x
}

/// A layer tag: the number in the operator's numbering and what the band is, right-aligned at
/// `right`, centred on `cy`.
fn tag(p: &Painter, theme: &Theme, right: f32, cy: f32, num: &str, what: &str, small: bool) {
    let size = if small { 10.0 } else { theme.font_size_small };
    let pad = if small { 5.0 } else { 7.0 };
    let n_w = width_of(p, num, size + 1.0);
    let w_w = width_of(p, what, size);
    let h = size + pad * 1.6;
    let rect = Rect::from_min_size(pos2(right - (n_w + w_w + pad * 3.0), cy - h / 2.0), vec2(n_w + w_w + pad * 3.0, h));
    p.rect_filled(rect, CornerRadius::same(4), theme.bg_primary());
    p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, theme.info()), StrokeKind::Inside);
    p.text(pos2(rect.left() + pad, cy), egui::Align2::LEFT_CENTER, num, FontId::proportional(size + 1.0), theme.info());
    p.text(pos2(rect.left() + pad * 2.0 + n_w, cy), egui::Align2::LEFT_CENTER, what, FontId::proportional(size), theme.text_primary());
}

/// A mouse pointer, painted (a snapshot has no cursor).
fn pointer(p: &Painter, theme: &Theme, at: Pos2) {
    let pts = vec![at, at + vec2(0.0, 19.0), at + vec2(5.0, 14.5), at + vec2(13.0, 13.0)];
    p.add(Shape::convex_polygon(pts, theme.text_strong(), Stroke::new(1.5_f32, theme.bg_primary())));
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Idle,
    Hover,
    Active,
}

/// One primary button at `x`; returns its rect.
fn primary_button(p: &Painter, theme: &Theme, x: f32, cy: f32, cat: &Cat, look: Look) -> Rect {
    let label = t(cat.name);
    let col = (cat.color)(theme);
    let ink = if look == Look::Idle { theme.text_secondary() } else { theme.text_strong() };
    let g = p.layout_no_wrap(label.to_string(), FontId::proportional(theme.font_size_body), ink);
    let w = 12.0 + ICON + 7.0 + g.size().x + 12.0;
    let rect = Rect::from_min_size(pos2(x, cy - PRIMARY_BTN_H / 2.0), vec2(w, PRIMARY_BTN_H));
    let (fill, stroke) = match look {
        Look::Active => (tint(col, 46), Stroke::new(2.0_f32, col)),
        Look::Hover => (tint(col, 26), Stroke::new(2.0_f32, theme.accent())),
        Look::Idle => (tint(col, 12), Stroke::new(1.0_f32, tint(col, 90))),
    };
    p.rect_filled(rect, CornerRadius::same(6), fill);
    p.rect_stroke(rect, CornerRadius::same(6), stroke, StrokeKind::Inside);
    let icon = Rect::from_min_size(pos2(rect.left() + 12.0, cy - ICON / 2.0), Vec2::splat(ICON));
    icons::paint_nav_icon(p, icon, cat.icon, ink);
    p.galley(pos2(icon.right() + 7.0, cy - g.size().y / 2.0), g, ink);
    rect
}

/// One phone primary button (the icon over its word) at `x`; returns its rect.
fn phone_primary_button(p: &Painter, theme: &Theme, x: f32, top: f32, cat: &Cat, active: bool) -> Rect {
    let col = (cat.color)(theme);
    let ink = if active { theme.text_strong() } else { theme.text_secondary() };
    let rect = Rect::from_min_size(pos2(x, top + 4.0), vec2(64.0, PHONE_PRIMARY_H - 8.0));
    p.rect_filled(rect, CornerRadius::same(6), tint(col, if active { 46 } else { 12 }));
    if active {
        p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(2.0_f32, col), StrokeKind::Inside);
    }
    let icon = Rect::from_min_size(pos2(rect.center().x - 11.0, rect.top() + 6.0), Vec2::splat(22.0));
    icons::paint_nav_icon(p, icon, cat.icon, ink);
    p.text(pos2(rect.center().x, rect.bottom() - 9.0), egui::Align2::CENTER_CENTER, t(cat.name), FontId::proportional(11.5), ink);
    rect
}

/// One secondary item at `x`; returns its width.
fn chip(p: &Painter, theme: &Theme, x: f32, cy: f32, h: f32, label: &str, col: Color32, active: bool, new: bool) -> f32 {
    let ink = if active { theme.text_strong() } else { theme.text_secondary() };
    let g = p.layout_no_wrap(label.to_string(), FontId::proportional(SECONDARY_FONT), ink);
    let new_w = if new { width_of(p, "new", 9.5) + 8.0 } else { 0.0 };
    let w = 12.0 + g.size().x + if new { 5.0 + new_w } else { 0.0 } + 12.0;
    let rect = Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w, h));
    if active {
        p.rect_filled(rect, CornerRadius::same(5), tint(col, 40));
        p.rect_filled(Rect::from_min_size(pos2(rect.left() + 6.0, rect.bottom() - 3.0), vec2(w - 12.0, 3.0)), CornerRadius::same(1), col);
    }
    p.galley(pos2(rect.left() + 12.0, cy - g.size().y / 2.0), g, ink);
    if new {
        let nr = Rect::from_min_size(pos2(rect.right() - 12.0 - new_w, cy - 7.0), vec2(new_w, 14.0));
        p.rect_filled(nr, CornerRadius::same(7), tint(theme.success(), 60));
        p.text(nr.center(), egui::Align2::CENTER_CENTER, "new", FontId::proportional(9.5), theme.text_strong());
    }
    w
}

// ── The bands ───────────────────────────────────────────────────────────────────────────────────

fn primary_row(ctx: &egui::Context, theme: &Theme, m: &Mock) -> Option<Rect> {
    let h = if m.phone { PHONE_PRIMARY_H } else { PRIMARY_H };
    let mut hovered = None;
    egui::TopBottomPanel::top("mock_primary")
        .exact_height(h)
        .frame(Frame::NONE.fill(theme.bg_card()))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let p = ui.painter().clone();
            let cy = full.center().y;
            if m.phone {
                // The row scrolls sideways under a thumb; All pages stays pinned at its right end.
                let menu_w = 52.0;
                let clip = Rect::from_min_max(full.min, pos2(full.right() - menu_w, full.bottom()));
                let pc = p.with_clip_rect(clip);
                let mut x = full.left() + 6.0;
                for (i, cat) in CATS.iter().enumerate() {
                    let r = phone_primary_button(&pc, theme, x, full.top(), cat, i == m.active);
                    x = r.right() + 4.0;
                }
                // A fade says the row goes on.
                for k in 0..10 {
                    let a = (k as f32 / 10.0 * 230.0) as u8;
                    let r = Rect::from_min_size(pos2(clip.right() - 30.0 + k as f32 * 3.0, full.top()), vec2(3.0, h));
                    p.rect_filled(r, CornerRadius::ZERO, tint(theme.bg_card(), a));
                }
                let mr = Rect::from_min_size(pos2(full.right() - menu_w + 4.0, full.top() + 4.0), vec2(menu_w - 8.0, h - 8.0));
                p.rect_filled(mr, CornerRadius::same(6), theme.bg_secondary());
                p.rect_stroke(mr, CornerRadius::same(6), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
                for k in 0..3 {
                    let y = mr.top() + 14.0 + k as f32 * 6.0;
                    p.line_segment([pos2(mr.center().x - 9.0, y), pos2(mr.center().x + 9.0, y)], Stroke::new(2.0_f32, theme.text_primary()));
                }
                p.text(pos2(mr.center().x, mr.bottom() - 9.0), egui::Align2::CENTER_CENTER, "All", FontId::proportional(10.5), theme.text_secondary());
                tag(&p, theme, full.right() - menu_w - 34.0, full.top() + 9.0, "10", "primary menu, scrolls", true);
                return;
            }
            let mut x = full.left() + 8.0;
            // Back and Forward, the browser's buttons (desktop only: the website has the browser's own).
            for (i, can) in [(0, true), (1, false)] {
                let r = Rect::from_min_size(pos2(x, cy - 16.0), vec2(32.0, 32.0));
                let ink = if can { theme.text_primary() } else { tint(theme.text_muted(), 110) };
                p.rect_filled(r, CornerRadius::same(6), theme.bg_secondary());
                let ir = r.shrink(8.0);
                if i == 0 { icons::paint_arrow_left(&p, ir, ink) } else { icons::paint_arrow_right(&p, ir, ink) }
                x = r.right() + 3.0;
            }
            x += 10.0;
            for (i, cat) in CATS.iter().enumerate() {
                let look = if i == m.active { Look::Active } else if m.preview == Some(i) { Look::Hover } else { Look::Idle };
                let r = primary_button(&p, theme, x, cy, cat, look);
                if m.preview == Some(i) {
                    hovered = Some(r);
                }
                if i == m.active {
                    // The active category's colour runs down into its secondary row.
                    p.rect_filled(Rect::from_min_size(pos2(r.left() + 8.0, full.bottom() - 3.0), vec2(r.width() - 16.0, 3.0)), CornerRadius::ZERO, (cat.color)(theme));
                }
                x = r.right() + 5.0;
            }
            // The right end: the presentation cycle, the notices bell and help, as today.
            let mut rx = full.right() - 8.0;
            for k in 0..3 {
                let r = Rect::from_min_size(pos2(rx - 32.0, cy - 16.0), vec2(32.0, 32.0));
                p.rect_filled(r, CornerRadius::same(6), theme.bg_secondary());
                match k {
                    0 => { p.text(r.center(), egui::Align2::CENTER_CENTER, "?", FontId::proportional(16.0), theme.text_primary()); }
                    1 => icons::paint_bell(&p, r.shrink(8.0), theme.text_primary()),
                    _ => { p.text(r.center(), egui::Align2::CENTER_CENTER, "Aa", FontId::proportional(12.0), theme.text_primary()); }
                }
                rx = r.left() - 4.0;
            }
            tag(&p, theme, rx - 12.0, cy, "10", "primary menu: the categories, each a page", false);
        });
    hovered
}

fn secondary_row(ctx: &egui::Context, theme: &Theme, m: &Mock) {
    let h = if m.phone { PHONE_SECONDARY_H } else { SECONDARY_H };
    let shown = m.preview.unwrap_or(m.active);
    let cat = &CATS[shown];
    let col = (cat.color)(theme);
    egui::TopBottomPanel::top("mock_secondary")
        .exact_height(h)
        .frame(Frame::NONE.fill(theme.bg_secondary()))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let p = ui.painter().clone();
            p.rect_filled(full, CornerRadius::ZERO, tint(col, if m.preview.is_some() { 22 } else { 12 }));
            let cy = full.center().y;
            let pc = p.with_clip_rect(full);
            let mut x = full.left() + if m.phone { 6.0 } else { 62.0 };
            for (i, &(key, new)) in cat.items.iter().enumerate() {
                let active = m.preview.is_none() && m.active_item == Some(i);
                let bh = if m.phone { 40.0 } else { SECONDARY_BTN_H };
                x += chip(&pc, theme, x, cy, bh, t(key), col, active, new) + if m.phone { 4.0 } else { 6.0 };
            }
            // A line in the shown category's colour closes the menus off from the page.
            p.rect_filled(Rect::from_min_size(pos2(full.left(), full.bottom() - 2.0), vec2(full.width(), 2.0)), CornerRadius::ZERO, tint(col, 200));
            if m.phone {
                tag(&p, theme, full.right() - 6.0, full.top() + 8.0, "20", "secondary menu, scrolls", true);
            } else if m.preview.is_some() {
                tag(&p, theme, full.right() - 12.0, cy, "20", "secondary menu: Play's pages while the pointer rests on Play; Chat's come back when it leaves", false);
            } else {
                tag(&p, theme, full.right() - 12.0, cy, "20", "secondary menu: this category's other pages", false);
            }
        });
}

/// The footer chat (layer 30), above the warning: added after `bottom_strip`, so egui stacks it
/// inside the warning.
fn footer(ctx: &egui::Context, theme: &Theme, m: &Mock) {
    let h = match m.footer {
        Footer::Off => return,
        Footer::Folded => 32.0,
        Footer::Unfolded => 210.0,
    };
    egui::TopBottomPanel::bottom("mock_footer")
        .exact_height(h)
        .frame(Frame::NONE.fill(theme.bg_card()))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let p = ui.painter().clone();
            p.line_segment([full.left_top(), full.right_top()], Stroke::new(1.0_f32, theme.border()));
            let small = theme.font_size_small;
            let body = theme.font_size_body;
            if m.footer == Footer::Folded {
                let cy = full.center().y;
                let mut x = full.left() + 12.0;
                let chan = "# global";
                let cw = width_of(&p, chan, body) + 16.0;
                let cr = Rect::from_min_size(pos2(x, cy - 11.0), vec2(cw, 22.0));
                p.rect_filled(cr, CornerRadius::same(4), tint(theme.accent(), 40));
                p.text(cr.center(), egui::Align2::CENTER_CENTER, chan, FontId::proportional(body), theme.text_strong());
                x = cr.right() + 10.0;
                p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, "Ana: The tomatoes are up. Who wants seed potatoes for next week?", FontId::proportional(body), theme.text_primary());
                let br = Rect::from_min_size(pos2(x + 560.0, cy - 9.0), vec2(70.0, 18.0));
                p.rect_filled(br, CornerRadius::same(9), theme.danger());
                p.text(br.center(), egui::Align2::CENTER_CENTER, "3 unread", FontId::proportional(small), theme.text_strong());
                let mut rx = full.right() - 12.0;
                for label in ["Open in Chat", "Unfold"] {
                    let w = width_of(&p, label, small) + 18.0;
                    let r = Rect::from_min_size(pos2(rx - w, cy - 11.0), vec2(w, 22.0));
                    p.rect_filled(r, CornerRadius::same(4), theme.bg_secondary());
                    p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
                    p.text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(small), theme.text_primary());
                    rx = r.left() - 6.0;
                }
                tag(&p, theme, rx - 12.0, cy, "30", "footer chat, folded", false);
                return;
            }
            // Unfolded: the channels as ordinary rows (#local among them), the messages, the box.
            let grip = Rect::from_center_size(pos2(full.center().x, full.top() + 4.0), vec2(48.0, 3.0));
            p.rect_filled(grip, CornerRadius::same(2), theme.border());
            let list = Rect::from_min_max(pos2(full.left() + 8.0, full.top() + 12.0), pos2(full.left() + 220.0, full.bottom() - 8.0));
            p.rect_filled(list, CornerRadius::same(6), theme.bg_secondary());
            let mut y = list.top() + 8.0;
            for (i, (name, unread)) in [("# global", "3"), ("# local", ""), ("# feedback", "1"), ("# announcements", "")].iter().enumerate() {
                let r = Rect::from_min_size(pos2(list.left() + 6.0, y), vec2(list.width() - 12.0, 30.0));
                if i == 0 {
                    p.rect_filled(r, CornerRadius::same(4), tint(theme.accent(), 46));
                }
                p.text(pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, *name, FontId::proportional(body), if i == 0 { theme.text_strong() } else { theme.text_secondary() });
                if !unread.is_empty() {
                    let b = Rect::from_center_size(pos2(r.right() - 16.0, r.center().y), vec2(20.0, 18.0));
                    p.rect_filled(b, CornerRadius::same(9), theme.danger());
                    p.text(b.center(), egui::Align2::CENTER_CENTER, *unread, FontId::proportional(small), theme.text_strong());
                }
                y = r.bottom() + 4.0;
            }
            let msgs = Rect::from_min_max(pos2(list.right() + 12.0, full.top() + 12.0), pos2(full.right() - 12.0, full.bottom() - 52.0));
            let mut my = msgs.top() + 4.0;
            for (who, said) in [
                ("Ben", "Morning all. The rain barrel filter from the How-to worked."),
                ("Ana", "The tomatoes are up. Who wants seed potatoes for next week?"),
                ("Shaostoul", "Me, two please. I can bring the drill on Thursday."),
            ] {
                let ww = width_of(&p, who, body);
                p.text(pos2(msgs.left(), my), egui::Align2::LEFT_TOP, who, FontId::proportional(body), theme.accent());
                my += words(&p, pos2(msgs.left() + ww + 10.0, my), said, body, theme.text_primary(), msgs.width() - ww - 10.0) + 8.0;
            }
            let bx = Rect::from_min_max(pos2(msgs.left(), full.bottom() - 44.0), pos2(full.right() - 12.0, full.bottom() - 10.0));
            p.rect_filled(bx, CornerRadius::same(6), theme.bg_primary());
            p.rect_stroke(bx, CornerRadius::same(6), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
            p.text(pos2(bx.left() + 12.0, bx.center().y), egui::Align2::LEFT_CENTER, "Write to # global", FontId::proportional(body), theme.text_muted());
            tag(&p, theme, full.right() - 12.0, full.top() + 22.0, "30", "footer chat, unfolded: its top edge drags; #local is an ordinary row", false);
        });
}

/// The six question cards, `cols` across, from `top`; returns the bottom.
fn question_cards(p: &Painter, theme: &Theme, left: f32, top: f32, width: f32, cols: usize, phone: bool) -> f32 {
    let qs: [(&str, &str, &str, &str); 6] = [
        ("front_door.q_who", "front_door.q_who_line", "nav.people", "nav.chat"),
        ("front_door.q_what", "front_door.q_what_line", "nav.apps", "front_door.dest_game"),
        ("front_door.q_when", "front_door.q_when_line", "nav.roadmap", "nav.devlog"),
        ("front_door.q_where", "front_door.q_where_line", "nav.servers", "nav.maps"),
        ("front_door.q_why", "front_door.q_why_line", "nav.mission", "front_door.dest_decide"),
        ("front_door.q_how", "front_door.q_how_line", "nav.how_to", "nav.library"),
    ];
    let gap = if phone { 8.0 } else { 14.0 };
    let cw = (width - gap * (cols as f32 - 1.0)) / cols as f32;
    let pad = if phone { 10.0 } else { 14.0 };
    let line_size = if phone { theme.font_size_small } else { theme.font_size_body };
    // Every card as tall as the tallest one needs: the question, its line, a gap, the links.
    let tallest_line = qs
        .iter()
        .map(|&(_, line, _, _)| p.layout(t(line).to_string(), FontId::proportional(line_size), theme.text_primary(), cw - pad * 2.0).size().y)
        .fold(0.0_f32, f32::max);
    let ch = pad * 2.0 + theme.font_size_heading * 1.3 + 4.0 + tallest_line + 14.0 + theme.font_size_small * 1.3;
    let mut bottom = top;
    for (i, &(q, line, opens, also)) in qs.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let rect = Rect::from_min_size(pos2(left + c as f32 * (cw + gap), top + r as f32 * (ch + gap)), vec2(cw, ch));
        p.rect_filled(rect, CornerRadius::same(theme.border_radius as u8), theme.bg_card());
        p.rect_stroke(rect, CornerRadius::same(theme.border_radius as u8), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
        let mut y = rect.top() + pad;
        y += words(p, pos2(rect.left() + pad, y), t(q), theme.font_size_heading, theme.accent(), cw - pad * 2.0) + 4.0;
        words(p, pos2(rect.left() + pad, y), t(line), line_size, theme.text_primary(), cw - pad * 2.0);
        let foot = format!("{}   ·   {}", t(opens), t(also));
        p.text(pos2(rect.left() + pad, rect.bottom() - pad), egui::Align2::LEFT_BOTTOM, foot, FontId::proportional(theme.font_size_small), theme.accent());
        bottom = rect.bottom();
    }
    bottom
}

fn page_overview(ctx: &egui::Context, theme: &Theme, m: &Mock) {
    egui::CentralPanel::default().frame(Frame::NONE.fill(theme.bg_panel())).show(ctx, |ui| {
        let r = ui.max_rect();
        let p = ui.painter().clone();
        let col_w = if m.phone { r.width() - 32.0 } else { 1100.0_f32.min(r.width() - 64.0) };
        let left = r.center().x - col_w / 2.0;
        let mut y = r.top() + if m.phone { 16.0 } else { 26.0 };
        y += words(&p, pos2(left, y), t("humanity.title"), theme.font_size_title, theme.text_strong(), col_w) + 2.0;
        y += words(&p, pos2(left, y), t("humanity.mission"), theme.font_size_body, theme.accent(), col_w) + if m.phone { 12.0 } else { 18.0 };
        if !m.phone {
            // The three bands of the overview (top-menu-and-game-pages.md 4b, B1): sample values.
            let bands: [(&str, Color32, [&str; 3]); 3] = [
                ("You", theme.nav_legacy_green(), ["3 tasks today; next: water the seedlings", "Next on your calendar: volunteering, Tuesday 8:00", "2 unread messages in # global"]),
                ("Your character (Example)", theme.nav_sim(), ["Food 72%   Water 48%: drink soon   Energy 80%", "Lives in a Cabin, c2, on united-humanity.us", "Next quest step: fix the water filter"]),
                ("Us", theme.nav_reality(), ["214 members, 12 online now", "2 open proposals to vote on", "Newest release: v0.1550.0, signed"]),
            ];
            let gap = 14.0;
            let bw = (col_w - gap * 2.0) / 3.0;
            for (i, (name, col, lines)) in bands.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(left + i as f32 * (bw + gap), y), vec2(bw, 128.0));
                p.rect_filled(rect, CornerRadius::same(theme.border_radius as u8), theme.bg_card());
                p.rect_filled(Rect::from_min_size(rect.min, vec2(4.0, rect.height())), CornerRadius::ZERO, *col);
                let mut ly = rect.top() + 12.0;
                ly += words(&p, pos2(rect.left() + 16.0, ly), name, theme.font_size_body + 1.0, theme.text_strong(), bw - 28.0) + 6.0;
                for l in lines {
                    ly += words(&p, pos2(rect.left() + 16.0, ly), l, theme.font_size_small + 1.0, theme.text_primary(), bw - 28.0) + 4.0;
                }
            }
            y += 128.0 + 26.0;
        }
        y += words(&p, pos2(left, y), "About HumanityOS", theme.font_size_heading, theme.text_strong(), col_w) + 10.0;
        y = question_cards(&p, theme, left, y, col_w, if m.phone { 2 } else { 3 }, m.phone) + if m.phone { 16.0 } else { 24.0 };
        y += words(&p, pos2(left, y), "What changed lately", theme.font_size_heading, theme.text_strong(), col_w) + 10.0;
        let n = if m.phone { 1 } else { 4 };
        let gap = 12.0;
        let cw = if m.phone { col_w * 0.82 } else { (col_w - gap * 3.0) / 4.0 };
        for (i, (v, head, signed)) in [
            ("v0.1550.0", "The website's front door: update cards, Start here and search", true),
            ("v0.1549.0", "Tasks of my own, kept on my devices and sealed", false),
            ("v0.1548.0", "Characters relaid out: who and where side by side", false),
            ("v0.1547.0", "The unfinished-software warning can be muted for a while", true),
        ].iter().take(n).enumerate() {
            let rect = Rect::from_min_size(pos2(left + i as f32 * (cw + gap), y), vec2(cw, 96.0));
            p.rect_filled(rect, CornerRadius::same(theme.border_radius as u8), theme.bg_card());
            p.rect_stroke(rect, CornerRadius::same(theme.border_radius as u8), Stroke::new(1.0_f32, if *signed { theme.success() } else { theme.border() }), StrokeKind::Inside);
            p.text(pos2(rect.left() + 12.0, rect.top() + 10.0), egui::Align2::LEFT_TOP, *v, FontId::proportional(theme.font_size_small + 1.0), theme.accent());
            if *signed {
                p.text(pos2(rect.right() - 12.0, rect.top() + 10.0), egui::Align2::RIGHT_TOP, "Signed", FontId::proportional(theme.font_size_small), theme.success());
            }
            words(&p, pos2(rect.left() + 12.0, rect.top() + 32.0), head, theme.font_size_small + 1.5, theme.text_primary(), cw - 24.0);
        }
        let label = if m.phone { "page: one column" } else { "page: one column (the Humanity overview)" };
        tag(&p, theme, r.right() - if m.phone { 6.0 } else { 16.0 }, r.top() + if m.phone { 12.0 } else { 18.0 }, "40+", label, m.phone);
    });
}

fn page_files_three(ctx: &egui::Context, theme: &Theme) {
    egui::CentralPanel::default().frame(Frame::NONE.fill(theme.bg_panel())).show(ctx, |ui| {
        let r = ui.max_rect().shrink(12.0);
        let p = ui.painter().clone();
        let body = theme.font_size_body;
        let small = theme.font_size_small;
        let lw = (r.width() * 0.2).clamp(260.0, 420.0);
        let rw = (r.width() * 0.22).clamp(280.0, 460.0);
        let a = Rect::from_min_max(r.min, pos2(r.left() + lw, r.bottom()));
        let c = Rect::from_min_max(pos2(r.right() - rw, r.top()), r.max);
        let b = Rect::from_min_max(pos2(a.right() + 12.0, r.top()), pos2(c.left() - 12.0, r.bottom()));
        for pane in [a, b, c] {
            p.rect_filled(pane, CornerRadius::same(theme.border_radius as u8), theme.bg_card());
            p.rect_stroke(pane, CornerRadius::same(theme.border_radius as u8), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
        }
        // The list: the places, then the items.
        let mut y = a.top() + 12.0;
        for (i, place) in ["Shared on united-humanity.us", "Shared files", "The HumanityOS folder", "Your folders"].iter().enumerate() {
            let row = Rect::from_min_size(pos2(a.left() + 8.0, y), vec2(a.width() - 16.0, 30.0));
            if i == 0 {
                p.rect_filled(row, CornerRadius::same(4), tint(theme.accent(), 40));
            }
            p.text(pos2(row.left() + 10.0, row.center().y), egui::Align2::LEFT_CENTER, *place, FontId::proportional(body), if i == 0 { theme.text_strong() } else { theme.text_secondary() });
            y = row.bottom() + 3.0;
        }
        y += 14.0;
        for (i, (name, who)) in [("Tomato trellis", "Ana"), ("Water filter housing", "Ben"), ("Solar panel wiring", "Shaostoul"), ("Seed calendar", "Ana"), ("Rain barrel stand", "Kai")].iter().enumerate() {
            let row = Rect::from_min_size(pos2(a.left() + 8.0, y), vec2(a.width() - 16.0, 58.0));
            if i == 0 {
                p.rect_stroke(row, CornerRadius::same(4), Stroke::new(2.0_f32, theme.accent()), StrokeKind::Inside);
            }
            let th = Rect::from_min_size(pos2(row.left() + 6.0, row.top() + 7.0), vec2(44.0, 44.0));
            p.rect_filled(th, CornerRadius::same(3), tint(theme.nav_legacy_green(), 60));
            p.text(pos2(th.right() + 10.0, row.top() + 8.0), egui::Align2::LEFT_TOP, *name, FontId::proportional(body), theme.text_primary());
            p.text(pos2(th.right() + 10.0, row.bottom() - 8.0), egui::Align2::LEFT_BOTTOM, *who, FontId::proportional(small), theme.text_muted());
            y = row.bottom() + 4.0;
        }
        // The item, fit to its pane.
        let pic = b.shrink(24.0);
        p.rect_filled(pic, CornerRadius::same(4), theme.bg_tertiary());
        p.text(pic.center(), egui::Align2::CENTER_CENTER, "Tomato trellis (the picture, fit to its pane)", FontId::proportional(theme.font_size_heading), theme.text_muted());
        // Its details.
        let mut dy = c.top() + 14.0;
        dy += words(&p, pos2(c.left() + 14.0, dy), "Tomato trellis", theme.font_size_heading, theme.text_strong(), c.width() - 28.0) + 10.0;
        for (k, v) in [("From", "Ana"), ("Kind", "A picture"), ("Category", "Growing food"), ("Picked by", "12 people"), ("Shared", "Today, 9:14")] {
            p.text(pos2(c.left() + 14.0, dy), egui::Align2::LEFT_TOP, k, FontId::proportional(small), theme.text_muted());
            p.text(pos2(c.left() + 120.0, dy), egui::Align2::LEFT_TOP, v, FontId::proportional(body), theme.text_primary());
            dy += 28.0;
        }
        tag(&p, theme, b.right() - 12.0, b.bottom() - 22.0, "40+", "page: three columns (Files: the list, the item, its details)", false);
    });
}

/// The full-screen popup over everything, an urgent call over it and a tooltip over that.
fn popup(ctx: &egui::Context, theme: &Theme) {
    let screen = ctx.screen_rect();
    let shade_id = Id::new("mock_shade");
    egui::Area::new(shade_id).order(Order::Middle).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(165));
        tag(p, theme, screen.right() - 24.0, screen.top() + 150.0, "5", "the shade: over everything, the menus and the warning too; a click on it closes the popup", false);
    });
    let win = Rect::from_center_size(screen.center(), vec2(620.0, 280.0));
    let win_id = Id::new("mock_popup");
    let mut remove_btn = Rect::NOTHING;
    egui::Area::new(win_id).order(Order::Middle).fixed_pos(win.min).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(win, CornerRadius::same(theme.border_radius as u8), theme.bg_card());
        p.rect_stroke(win, CornerRadius::same(theme.border_radius as u8), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
        let mut y = win.top() + 22.0;
        y += words(p, pos2(win.left() + 24.0, y), "Remove this file from the Files page?", theme.font_size_heading, theme.text_strong(), win.width() - 48.0) + 12.0;
        words(p, pos2(win.left() + 24.0, y), "Tomato trellis is taken off the Files page. The picture stays in the chat message it came from.", theme.font_size_body, theme.text_primary(), win.width() - 48.0);
        let by = win.bottom() - 58.0;
        let cancel = Rect::from_min_size(pos2(win.right() - 24.0 - 120.0 - 12.0 - 120.0, by), vec2(120.0, 36.0));
        remove_btn = Rect::from_min_size(pos2(win.right() - 24.0 - 120.0, by), vec2(120.0, 36.0));
        p.rect_filled(cancel, CornerRadius::same(6), theme.bg_secondary());
        p.rect_stroke(cancel, CornerRadius::same(6), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
        p.text(cancel.center(), egui::Align2::CENTER_CENTER, "Cancel", FontId::proportional(theme.font_size_body), theme.text_primary());
        p.rect_filled(remove_btn, CornerRadius::same(6), theme.danger());
        p.text(remove_btn.center(), egui::Align2::CENTER_CENTER, "Remove", FontId::proportional(theme.font_size_body), theme.text_strong());
        tag(p, theme, win.right(), win.top() - 16.0, "4", "the popup window", false);
        tag(p, theme, cancel.left() - 10.0, cancel.center().y, "3", "its controls", false);
    });
    ctx.move_to_top(LayerId::new(Order::Middle, win_id));
    // Urgent (layer 2): over an open popup, never taking its typing.
    let card = Rect::from_center_size(pos2(screen.center().x, screen.top() + 52.0), vec2(470.0, 64.0));
    egui::Area::new(Id::new("mock_urgent")).order(Order::Foreground).fixed_pos(card.min).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(card, CornerRadius::same(8), theme.bg_card());
        p.rect_stroke(card, CornerRadius::same(8), Stroke::new(2.0_f32, theme.success()), StrokeKind::Inside);
        p.text(pos2(card.left() + 18.0, card.center().y), egui::Align2::LEFT_CENTER, "Ana is calling", FontId::proportional(theme.font_size_body + 1.0), theme.text_strong());
        let dec = Rect::from_min_size(pos2(card.right() - 14.0 - 96.0, card.center().y - 17.0), vec2(96.0, 34.0));
        let ans = Rect::from_min_size(pos2(dec.left() - 10.0 - 96.0, dec.top()), vec2(96.0, 34.0));
        p.rect_filled(ans, CornerRadius::same(6), theme.success());
        p.text(ans.center(), egui::Align2::CENTER_CENTER, "Answer", FontId::proportional(theme.font_size_body), theme.text_strong());
        p.rect_filled(dec, CornerRadius::same(6), theme.danger());
        p.text(dec.center(), egui::Align2::CENTER_CENTER, "Decline", FontId::proportional(theme.font_size_body), theme.text_strong());
        tag(p, theme, card.left() - 14.0, card.center().y, "2", "urgent: a call or an emergency alert, over a popup", false);
    });
    // A tooltip (layer 1), nearest of all.
    let tip = Rect::from_min_size(remove_btn.center() + vec2(-150.0, 30.0), vec2(330.0, 62.0));
    egui::Area::new(Id::new("mock_tooltip")).order(Order::Tooltip).fixed_pos(tip.min).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(tip, CornerRadius::same(4), theme.bg_primary());
        p.rect_stroke(tip, CornerRadius::same(4), Stroke::new(1.0_f32, theme.border()), StrokeKind::Inside);
        words(p, tip.min + vec2(10.0, 8.0), "Takes Tomato trellis off the Files page. You can put it back from the chat message.", theme.font_size_small + 1.0, theme.text_primary(), tip.width() - 20.0);
        tag(p, theme, tip.right() + 210.0, tip.center().y, "1", "a tooltip: nearest of all", false);
        pointer(p, theme, remove_btn.center() + vec2(14.0, 4.0));
    });
}

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, m: Mock) {
    state.active_page = GuiPage::Humanity;
    let hovered = primary_row(ctx, theme, &m);
    secondary_row(ctx, theme, &m);
    // The real unfinished-software warning, outermost at the bottom (layer 21), then the footer.
    crate::gui::bottom_strip::draw_for_version(ctx, theme, state, "0.1550.0");
    let strip_h = crate::gui::bottom_strip::height(ctx);
    footer(ctx, theme, &m);
    match m.page {
        Page::Overview => page_overview(ctx, theme, &m),
        Page::FilesThree => page_files_three(ctx, theme),
    }
    // The warning's tag, in the panels' own layer, so a popup's shade covers it as it covers them.
    // It stands just above the bands under the page (the footer, then the warning), pointing down,
    // so it never covers the warning's own words.
    let screen = ctx.screen_rect();
    if strip_h > 0.0 {
        let p = ctx.layer_painter(LayerId::background());
        let footer_h = match m.footer {
            Footer::Off => 0.0,
            Footer::Folded => 32.0,
            Footer::Unfolded => 210.0,
        };
        let words_w = if m.phone {
            "the warning (the bottom band)"
        } else {
            "the unfinished-software warning (the bottom line): nearer than the footer, which never covers it"
        };
        let y = screen.bottom() - strip_h - footer_h - if m.phone { 12.0 } else { 18.0 };
        tag(&p, theme, screen.right() - if m.phone { 6.0 } else { 16.0 }, y, "21", words_w, m.phone);
    }
    if let Some(r) = hovered {
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("mock_pointer")));
        pointer(&p, theme, r.center() + vec2(6.0, 2.0));
    }
    if m.popup {
        popup(ctx, theme);
    }
}

const TV_OVERVIEW: Mock = Mock { phone: false, active: 0, active_item: None, preview: None, page: Page::Overview, footer: Footer::Folded, popup: false };

pub(crate) fn tv() {
    render_page_png("nav_double_tv", 2560, 1440, |ctx, theme, state| frame(ctx, theme, state, TV_OVERVIEW));
}

pub(crate) fn tv_hover() {
    let m = Mock { active: 2, active_item: Some(2), preview: Some(1), page: Page::FilesThree, footer: Footer::Unfolded, ..TV_OVERVIEW };
    render_page_png("nav_double_tv_hover", 2560, 1440, move |ctx, theme, state| frame(ctx, theme, state, m));
}

pub(crate) fn tv_popup() {
    let m = Mock { popup: true, ..TV_OVERVIEW };
    render_page_png("nav_double_tv_popup", 2560, 1440, move |ctx, theme, state| frame(ctx, theme, state, m));
}

pub(crate) fn laptop() {
    render_page_png("nav_double_laptop", 1280, 720, |ctx, theme, state| frame(ctx, theme, state, TV_OVERVIEW));
}

pub(crate) fn phone() {
    let m = Mock { phone: true, footer: Footer::Off, ..TV_OVERVIEW };
    render_page_png("nav_double_phone", 412, 915, move |ctx, theme, state| frame(ctx, theme, state, m));
}
