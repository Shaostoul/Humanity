//! Map label placement: the greedy collision pass cartography uses for point
//! and line features, so the Maps views never print one name on top of
//! another (2026-09-27 snapshot review: star names stacked on each other in
//! the Galaxy view, street names on the Planet view).
//!
//! The technique is the standard one for interactive maps. Labels are offered
//! in PRIORITY order (the brightest star, the biggest road first). Each label
//! brings a short list of candidate rectangles in PREFERENCE order (for a
//! point: right of it, then left, above, below). The first candidate that is
//! inside the map and clear of everything already placed wins; a label with
//! no clear candidate is dropped, so the more important name keeps its place
//! and the less important one gives way. Fixed things a label must not cover
//! (a legend, the attribution, the dot of a star) are `reserve`d up front.
//!
//! Greedy is deliberate. Optimal placement is NP-hard; greedy in priority
//! order is what production map renderers ship because it is stable (the same
//! view always labels the same way) and cheap (a linear scan over at most a
//! few hundred rectangles per frame here).
//!
//! The web page mirrors this in `web/shared/label-placer.js`; keep the two in
//! step.

use egui::{Align2, Pos2, Rect, Vec2};

/// The clear gap the Maps views keep between two names: a little wider than
/// a space at map-label sizes, so two neighbouring names never read as one
/// ("Sirius Alpha Centauri B" was two labels 1 px apart). The web twin uses
/// the same number.
pub const LABEL_GAP: f32 = 3.0;

/// The labels already on the map, plus anything reserved, inside `bounds`.
pub struct LabelPlacer {
    bounds: Rect,
    /// Every placed or reserved rectangle, grown by `pad` so neighbouring
    /// labels keep a little air between them.
    taken: Vec<Rect>,
    pad: f32,
}

impl LabelPlacer {
    /// An empty map. `bounds` is where labels may go (a label must lie wholly
    /// inside it); `pad` is the clear gap kept around each placed label.
    pub fn new(bounds: Rect, pad: f32) -> Self {
        Self { bounds, taken: Vec::new(), pad }
    }

    /// Mark an area no label may cover, keeping the label gap around it (a
    /// placed name, a legend, the attribution).
    pub fn reserve(&mut self, r: Rect) {
        self.taken.push(r.expand(self.pad));
    }

    /// Mark an obstacle held to its exact outline, with no gap around it: a
    /// point feature's dot. Its own label already sits a `gap` away (see
    /// `point_candidates`); padding the dot as well would block every one of
    /// that label's candidates whenever the pad reached the gap.
    pub fn reserve_exact(&mut self, r: Rect) {
        self.taken.push(r);
    }

    /// Whether `r` is inside the bounds and clear of everything placed.
    pub fn is_free(&self, r: Rect) -> bool {
        self.bounds.contains_rect(r) && !self.taken.iter().any(|t| t.intersects(r))
    }

    /// Place the label at the first free candidate, in the order given, and
    /// return where it went; `None` when every candidate is blocked (the
    /// label is dropped).
    pub fn place(&mut self, candidates: impl IntoIterator<Item = Rect>) -> Option<Rect> {
        let spot = candidates.into_iter().find(|r| self.is_free(*r))?;
        self.reserve(spot);
        Some(spot)
    }

    /// How many rectangles are placed or reserved.
    pub fn len(&self) -> usize {
        self.taken.len()
    }

    /// True when nothing is placed or reserved yet.
    pub fn is_empty(&self) -> bool {
        self.taken.is_empty()
    }
}

/// The four standard candidate positions for labelling a point feature
/// (a star, a city dot) of radius `r` at `p` with a label of `size`, in the
/// usual order of preference: right of the point, left of it, centred above,
/// centred below. `gap` is the clear space between the dot and its label.
pub fn point_candidates(p: Pos2, r: f32, size: Vec2, gap: f32) -> [Rect; 4] {
    let d = r + gap;
    [
        Align2::LEFT_CENTER.anchor_size(p + Vec2::new(d, 0.0), size),
        Align2::RIGHT_CENTER.anchor_size(p - Vec2::new(d, 0.0), size),
        Align2::CENTER_BOTTOM.anchor_size(p - Vec2::new(0.0, d), size),
        Align2::CENTER_TOP.anchor_size(p + Vec2::new(0.0, d), size),
    ]
}

/// Anchor points along a polyline (a road) at the given fractions of its
/// length, for labelling a line feature: a name centred on one of them.
/// Offering several (the middle first, then either side) lets a street name
/// slide along its street to clear a crossing one. Empty for a polyline with
/// fewer than two points or no length.
pub fn along_polyline(points: &[Pos2], fractions: &[f32]) -> Vec<Pos2> {
    if points.len() < 2 {
        return Vec::new();
    }
    let seg_len: Vec<f32> = points.windows(2).map(|w| (w[1] - w[0]).length()).collect();
    let total: f32 = seg_len.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    fractions
        .iter()
        .map(|&f| {
            let mut want = f.clamp(0.0, 1.0) * total;
            for (i, &len) in seg_len.iter().enumerate() {
                if want <= len || i == seg_len.len() - 1 {
                    let t = if len > 0.0 { (want / len).clamp(0.0, 1.0) } else { 0.0 };
                    return points[i] + (points[i + 1] - points[i]) * t;
                }
                want -= len;
            }
            points[points.len() - 1]
        })
        .collect()
}

/// Screen length of a polyline, the usual tiebreak for which road of a class
/// earns its name first (the longer one).
pub fn polyline_length(points: &[Pos2]) -> f32 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> LabelPlacer {
        LabelPlacer::new(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0)), 1.0)
    }

    #[test]
    fn the_first_label_takes_its_first_choice() {
        let mut m = map();
        let want = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 10.0));
        assert_eq!(m.place([want]), Some(want));
    }

    #[test]
    fn a_colliding_label_moves_to_its_next_candidate() {
        let mut m = map();
        let a = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 10.0));
        m.place([a]);
        let clash = Rect::from_min_size(Pos2::new(30.0, 12.0), Vec2::new(40.0, 10.0));
        let clear = Rect::from_min_size(Pos2::new(30.0, 40.0), Vec2::new(40.0, 10.0));
        assert_eq!(m.place([clash, clear]), Some(clear));
    }

    #[test]
    fn a_label_with_no_clear_candidate_is_dropped() {
        let mut m = map();
        let a = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 10.0));
        m.place([a]);
        assert_eq!(m.place([a]), None, "the same spot twice must not stack");
        assert_eq!(m.len(), 1, "a dropped label reserves nothing");
    }

    #[test]
    fn the_padding_keeps_touching_labels_apart() {
        let mut m = map();
        let a = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 10.0));
        m.place([a]);
        // Flush against `a`'s right edge: inside the 1 px pad, so blocked.
        let flush = Rect::from_min_size(Pos2::new(50.0, 10.0), Vec2::new(40.0, 10.0));
        assert!(!m.is_free(flush));
        let spaced = Rect::from_min_size(Pos2::new(52.0, 10.0), Vec2::new(40.0, 10.0));
        assert!(m.is_free(spaced));
    }

    #[test]
    fn labels_stay_inside_the_map_and_off_reserved_areas() {
        let mut m = map();
        let off_edge = Rect::from_min_size(Pos2::new(180.0, 10.0), Vec2::new(40.0, 10.0));
        assert!(!m.is_free(off_edge), "a label cut by the map edge is not placed");
        let footer = Rect::from_min_size(Pos2::new(0.0, 85.0), Vec2::new(200.0, 15.0));
        m.reserve(footer);
        let over_footer = Rect::from_min_size(Pos2::new(10.0, 86.0), Vec2::new(40.0, 10.0));
        assert!(!m.is_free(over_footer));
    }

    #[test]
    fn a_reserved_dot_never_blocks_its_own_label() {
        // The Maps views reserve every labelled dot, then place the names.
        // With the dot padded by the label gap, a name `gap` away from its
        // own dot collided with it and EVERY star lost its name. The dot is
        // held to its exact outline instead.
        let mut m = LabelPlacer::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0)),
            LABEL_GAP,
        );
        let p = Pos2::new(100.0, 50.0);
        let r = 3.0;
        m.reserve_exact(Rect::from_center_size(p, Vec2::splat(2.0 * r)));
        let c = point_candidates(p, r, Vec2::new(30.0, 10.0), 2.0);
        assert_eq!(m.place(c), Some(c[0]));
    }

    #[test]
    fn point_candidates_sit_clear_of_the_dot_in_preference_order() {
        let p = Pos2::new(100.0, 50.0);
        let c = point_candidates(p, 3.0, Vec2::new(30.0, 10.0), 2.0);
        let dot = Rect::from_center_size(p, Vec2::splat(6.0));
        for r in c {
            assert!(!r.intersects(dot.shrink(0.01)), "{r:?} covers the dot");
        }
        assert!(c[0].min.x > p.x, "right first");
        assert!(c[1].max.x < p.x, "then left");
        assert!(c[2].max.y < p.y, "then above");
        assert!(c[3].min.y > p.y, "then below");
    }

    #[test]
    fn along_polyline_walks_by_length_not_by_vertex_count() {
        // A long first segment and a short second one: the middle of the
        // LENGTH is inside the first segment, even though the middle vertex
        // is the corner.
        let pts = [Pos2::new(0.0, 0.0), Pos2::new(90.0, 0.0), Pos2::new(90.0, 10.0)];
        let mid = along_polyline(&pts, &[0.5]);
        assert_eq!(mid.len(), 1);
        assert!((mid[0] - Pos2::new(50.0, 0.0)).length() < 1e-3, "{:?}", mid[0]);
        assert!((polyline_length(&pts) - 100.0).abs() < 1e-3);
        assert!(along_polyline(&pts[..1], &[0.5]).is_empty());
    }
}
