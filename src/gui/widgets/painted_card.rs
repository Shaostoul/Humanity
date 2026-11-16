//! A CARD PAINTED ON A LAYER: a frame and lines of words, drawn with the layer's painter
//! (`layers::painter`), so it registers no widget and every click goes through it to whatever is
//! under it (2026-11-15, the follow-up to BUG-316).
//!
//! Why not an egui Area with `interactable(false)`: egui 0.31 gives every Area a widget rect the
//! size of what it holds, `interactable(false)` included, and each label in it one more; its hit
//! test stops at the topmost layer with a widget over the pointer, so an Area on the tooltip
//! layer (gui/layers.rs layer 1: tooltips, hints, toasts) took the click of whatever stood under
//! it. Seven garden notices buried a new person's privacy window that way (BUG-316), and the
//! F2/F3/F4 readouts, the planet card, the drop notes and the hover tips did the same. A painter
//! draws in the layer and registers nothing, like the toasts and the HUD (BUG-076), and
//! `tests/layer_lint.rs` refuses an Area or a Window built on the tooltip layer.
//!
//! What a card holds is laid out here, the way a `Ui` would lay it out (egui's own `Frame` for its
//! look and margins, the style's row height and spacing), so a readout keeps what it shows and
//! where; but nothing in it can be pressed. Anything that must take a click is not a tooltip: it
//! belongs on the layer the order gives clickable things (a page's window, a popup).
//!
//! The rect each card was painted in is kept for the frame under its id ([`rect`]): a painted card
//! has no Area whose rect egui keeps, so a test or a check that looks for it reads this.
//!
//! COST: the same text layout a `Ui` does, no widget, no memory beyond one rect per card a frame.

use egui::{Align2, Color32, Context, FontId, Id, Painter, Pos2, Rect, Vec2};

use crate::gui::layers::{self, Layer};

/// Where a card stands on the screen.
#[derive(Debug, Clone, Copy)]
pub enum Place {
    /// As egui's `Area::anchor(align, offset)` puts an Area: the card's `align` corner at the
    /// screen's, moved by `offset`.
    Anchor(Align2, Vec2),
    /// The card's `pivot` corner at `pos` (egui's `Area::fixed_pos` with `pivot`).
    At(Pos2, Align2),
}

/// Words to paint: the text, its font and its colour.
#[derive(Debug, Clone)]
pub struct Words {
    text: String,
    font: FontId,
    color: Color32,
}

impl Words {
    pub fn new(text: impl Into<String>, font: FontId, color: Color32) -> Self {
        Words { text: text.into(), font, color }
    }
}

/// One line of a card, top to bottom.
enum Line<'a> {
    /// Words across the card, wrapped to its width.
    Text(Words),
    /// Words on the left and words on the right of one row (a readout's label and its value).
    Pair(Words, Words),
    /// Empty room, no spacing of its own (egui's `add_space`).
    Space(f32),
    /// A thin line across (egui's `separator`).
    Separator,
    /// A block of this size, painted by the closure in the rect it is given (a graph, a badge row).
    Block(Vec2, Box<dyn FnOnce(&Painter, Rect) + 'a>),
}

/// A card to paint: build it line by line, then [`PaintedCard::paint`] it.
pub struct PaintedCard<'a> {
    frame: egui::Frame,
    width: Option<f32>,
    max_width: f32,
    gap: Option<f32>,
    lines: Vec<Line<'a>>,
}

impl<'a> PaintedCard<'a> {
    /// A card with egui's `frame` for its look (fill, stroke, corners, shadow and inner margin;
    /// `egui::Frame::popup(style)` is a tooltip's), as wide as its widest line.
    pub fn new(frame: egui::Frame) -> Self {
        PaintedCard { frame, width: None, max_width: f32::INFINITY, gap: None, lines: Vec::new() }
    }

    /// Exactly this wide inside its margins (egui's `set_width`).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// At most this wide inside its margins: longer words wrap (egui's `set_max_width`).
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// The room between lines; the style's item spacing when not set.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Words across the card, wrapped to its width.
    pub fn text(mut self, words: Words) -> Self {
        self.lines.push(Line::Text(words));
        self
    }

    /// Words on the left and on the right of one row.
    pub fn pair(mut self, left: Words, right: Words) -> Self {
        self.lines.push(Line::Pair(left, right));
        self
    }

    /// Empty room of this height.
    pub fn space(mut self, height: f32) -> Self {
        self.lines.push(Line::Space(height));
        self
    }

    /// A thin line across.
    pub fn separator(mut self) -> Self {
        self.lines.push(Line::Separator);
        self
    }

    /// A block of `size`, painted by `paint` in its rect.
    pub fn block(mut self, size: Vec2, paint: impl FnOnce(&Painter, Rect) + 'a) -> Self {
        self.lines.push(Line::Block(size, Box::new(paint)));
        self
    }

    /// Paint the card on `layer` at `place`, kept on the screen, under `id`; where its frame went.
    pub fn paint(self, ctx: &Context, id: Id, layer: Layer, place: Place) -> Rect {
        let style = ctx.style();
        let gap = self.gap.unwrap_or(style.spacing.item_spacing.y);
        let pair_gap = style.spacing.item_spacing.x;
        let row_min = style.spacing.interact_size.y;
        let wrap = self.width.unwrap_or(self.max_width);
        let sep_stroke = style.visuals.widgets.noninteractive.bg_stroke;
        const SEPARATOR_ROOM: f32 = 6.0; // egui's `Separator::default().spacing`

        // Lay every line out first: its size is known before the card is placed.
        enum Laid<'b> {
            Text(std::sync::Arc<egui::Galley>),
            Pair(std::sync::Arc<egui::Galley>, std::sync::Arc<egui::Galley>, f32),
            Space(f32),
            Separator,
            Block(Vec2, Box<dyn FnOnce(&Painter, Rect) + 'b>),
        }
        let laid: Vec<Laid> = ctx.fonts(|f| {
            self.lines
                .into_iter()
                .map(|line| match line {
                    Line::Text(w) => Laid::Text(f.layout(w.text, w.font, w.color, wrap)),
                    Line::Pair(l, r) => {
                        let l = f.layout_no_wrap(l.text, l.font, l.color);
                        let r = f.layout_no_wrap(r.text, r.font, r.color);
                        let h = l.size().y.max(r.size().y).max(row_min);
                        Laid::Pair(l, r, h)
                    }
                    Line::Space(h) => Laid::Space(h),
                    Line::Separator => Laid::Separator,
                    Line::Block(size, paint) => Laid::Block(size, paint),
                })
                .collect()
        });
        let natural_w = laid
            .iter()
            .map(|l| match l {
                Laid::Text(g) => g.size().x,
                Laid::Pair(a, b, _) => a.size().x + pair_gap + b.size().x,
                Laid::Block(size, _) => size.x,
                Laid::Space(_) | Laid::Separator => 0.0,
            })
            .fold(0.0_f32, f32::max);
        let inner_w = self.width.unwrap_or(natural_w.min(self.max_width));
        let height_of = |l: &Laid| match l {
            Laid::Text(g) => g.size().y,
            Laid::Pair(_, _, h) => *h,
            Laid::Space(h) => *h,
            Laid::Separator => SEPARATOR_ROOM,
            Laid::Block(size, _) => size.y,
        };
        // egui's placing: spacing after every widget, none of its own for `add_space`, and none
        // after the last.
        let mut inner_h = 0.0_f32;
        let mut trailing = 0.0_f32;
        for l in &laid {
            inner_h += height_of(l);
            trailing = if matches!(l, Laid::Space(_)) { 0.0 } else { gap };
            inner_h += trailing;
        }
        inner_h -= trailing;

        let margin = self.frame.total_margin();
        let outer = Vec2::new(inner_w, inner_h) + margin.sum();
        let screen = ctx.screen_rect();
        let mut frame_rect = match place {
            Place::Anchor(align, offset) => align.align_size_within_rect(outer, screen).translate(offset),
            Place::At(pos, pivot) => pivot.anchor_size(pos, outer),
        };
        // Kept on the screen, as egui keeps an Area (`constrain`): pushed in from the edge it
        // crosses, and to the left or top edge when it is bigger than the screen.
        let shift = |lo: f32, hi: f32, min: f32, max: f32| {
            if max - min > hi - lo {
                lo - min
            } else {
                (lo - min).max(0.0) + (hi - max).min(0.0)
            }
        };
        frame_rect = frame_rect.translate(Vec2::new(
            shift(screen.left(), screen.right(), frame_rect.left(), frame_rect.right()),
            shift(screen.top(), screen.bottom(), frame_rect.top(), frame_rect.bottom()),
        ));
        let content = Rect::from_min_max(frame_rect.min + margin.left_top(), frame_rect.max - margin.right_bottom());

        let painter = layers::painter(ctx, id, layer);
        painter.add(self.frame.paint(content));
        let mut y = content.top();
        for l in laid {
            let h = height_of(&l);
            let after = if matches!(l, Laid::Space(_)) { 0.0 } else { gap };
            match l {
                Laid::Text(g) => painter.galley(egui::pos2(content.left(), y), g, Color32::PLACEHOLDER),
                Laid::Pair(a, b, h) => {
                    painter.galley(egui::pos2(content.left(), y + (h - a.size().y) / 2.0), a, Color32::PLACEHOLDER);
                    painter.galley(egui::pos2(content.right() - b.size().x, y + (h - b.size().y) / 2.0), b, Color32::PLACEHOLDER);
                }
                Laid::Space(_) => {}
                Laid::Separator => {
                    painter.hline(content.left()..=content.right(), y + SEPARATOR_ROOM / 2.0, sep_stroke);
                }
                Laid::Block(size, paint) => paint(&painter, Rect::from_min_size(egui::pos2(content.left(), y), size)),
            }
            y += h + after;
        }
        // The pass is read BEFORE `data_mut`: egui's context lock is not re-entrant, and reading
        // it inside hung the first run of the tests (a deadlock, 2026-11-15).
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(rect_key(id), (pass, frame_rect)));
        frame_rect
    }
}

fn rect_key(id: Id) -> Id {
    id.with("painted_card_rect")
}

/// Where the card painted under `id` stood, when it was painted this frame or the last.
pub fn rect(ctx: &Context, id: Id) -> Option<Rect> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, Rect)>(rect_key(id))).filter(|(pass, _)| pass + 1 >= now).map(|(_, r)| r)
}
