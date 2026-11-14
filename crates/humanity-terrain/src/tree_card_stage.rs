//! THE TREE CARD STAGE: its supply and its counter (BUG-301, 2026-11-14).
//!
//! Tree cards (the patch-baked sprite cards that carry a forest from where the 3D models end to
//! the "Tree silhouette distance" setting) are baked ONLY into patches at depth >=
//! `TREE_MIN_DEPTH` (15, patches about 215 m across): the `want_trees` gate in the patch builder.
//! The selection's depth cap without the 460 m tile tier is `MAX_PATCH_DEPTH` (13), two levels
//! short. So a game without the tile tier drew NO card stage at all: every release (a release
//! never carries the tiles), every worktree and rig, and, once BUG-287 stopped an empty
//! `data/planets/earth_tiles` folder counting as the tier on 2026-11-09, the operator's game and
//! the main checkout's rig too. Past the near models the forest simply ended, and the only
//! sessions that ever saw the stage were the ones the empty folder had been fooling.
//!
//! THE SUPPLY ([`CardFloor`]). Within the card distance the cap is raised to the card depth, so
//! the ground the stage needs exists whether or not the tile tier is installed. The screen error
//! still decides whether a node splits (the floor only lifts the CAP), the leaf budget still binds,
//! and `far_trees::effective_card_far_m` still pulls the cutoff back to the reach the walk really
//! built, so a budget that runs out first shortens the stage honestly instead of promising cards.
//! Past the card distance the cap is the planet's own, so base-only ground far away costs nothing
//! new. Off where no tree can grow (the forest slider at 0, or a body whose data says its air is
//! not breathable: the planet-level stand-in for "the vegetation pass can grow trees here", since
//! the bake itself decides per tree from the imagery's colour and only Earth and Agartha grow any).
//!
//! THE COUNTER ([`CardTally`]). What a frame's draw list carries: the drawn patches, the ones that
//! carry cards, how many trees they carry as cards, and how many of those sit in patches that
//! overlap the window the fragment stage keeps (between the hide radius and the far cutoff). It
//! is what proved this bug (zero card patches with the deepest drawn patch at 13), it rides out on
//! every screenshot's done file, and probe-sweep fails a vantage whose `min_tree_cards` it misses.

use glam::DVec3;

use crate::planet_chunks::{patch_edge_arc_m, ChunkState, TREE_MIN_DEPTH};

/// Where the selection may refine past its own depth cap so the card stage has ground to stand
/// on: to `depth` for every patch whose nearest point is within `within_m` of the camera.
/// `depth` 0 (the default) is off.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CardFloor {
    pub depth: u8,
    pub within_m: f64,
}

impl CardFloor {
    /// The floor the frame loop asks for: the card depth out to the card distance (`card_far_m`,
    /// the Settings request, NOT the clamped cutoff, which is derived from the reach this floor
    /// creates), or off when nothing could stand there.
    pub fn for_stage(grows_trees: bool, tree_density: f32, card_far_m: f32) -> Self {
        if !grows_trees || !(tree_density > 0.0) || !(card_far_m > 0.0) {
            return Self::default();
        }
        Self { depth: TREE_MIN_DEPTH, within_m: card_far_m as f64 }
    }

    /// The depth cap for a patch whose nearest point is `dist_m` from the camera, given the cap it
    /// would otherwise have: never lowered, raised to the floor inside its reach.
    #[inline]
    pub fn cap(&self, cap: u8, dist_m: f64) -> u8 {
        if self.depth > cap && dist_m <= self.within_m {
            self.depth
        } else {
            cap
        }
    }
}

/// One frame's tree card stage, read from the drawn patch set (see the module header).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardTally {
    /// Patches in the draw list.
    pub drawn_patches: u32,
    /// Of those, the ones carrying at least one tree card.
    pub card_patches: u32,
    /// Trees the drawn patches carry as cards.
    pub cards: u32,
    /// Of those, the trees in patches that overlap the kept window (hide radius to far cutoff):
    /// the cards the fragment stage's distance test lets through, to a patch's width.
    pub cards_in_window: u32,
    /// The deepest drawn patch (13 = base-only ground, which carries no card).
    pub max_drawn_depth: u8,
}

/// Count `cs`'s drawn set as seen from its last selection's camera. `hide_m` and `far_m` are the
/// renderer's card window (`tree_card_hide_m`, `tree_card_far_m`); `radius_m` sizes each patch.
pub fn tally(cs: &ChunkState, radius_m: f64, hide_m: f32, far_m: f32) -> CardTally {
    tally_from(cs, cs.last_sel_cam, radius_m, hide_m, far_m)
}

/// [`tally`] from an explicit camera (planet-local, unrotated, metres).
pub fn tally_from(cs: &ChunkState, cam_local: DVec3, radius_m: f64, hide_m: f32, far_m: f32) -> CardTally {
    let mut t = CardTally::default();
    for id in &cs.last_drawn {
        t.drawn_patches += 1;
        t.max_drawn_depth = t.max_drawn_depth.max(id.depth);
        let Some(e) = cs.cache.get(id) else { continue };
        if e.card_trees == 0 {
            continue;
        }
        t.card_patches += 1;
        t.cards += e.card_trees;
        // The patch's ground: its anchor's direction at the middle of its measured radial band.
        // NOT the anchor itself, which sits on the sea-level sphere (`build_patch_mesh`: the
        // corners' centroid times the planet radius), so on land it lies below the ground by the
        // elevation: 1.3 km at Fuji, which put every card patch past the far cutoff from a camera
        // 350 m up and read 0 in the window over a full forest (the first boot, 2026-11-14).
        // Every point of a patch lies within about one edge length of that point (the edge is the
        // triangle's longest extent), so the patch overlaps the window when [d - edge, d + edge]
        // meets [hide, far].
        let ground = e.anchor.normalize_or_zero() * (0.5 * (e.band.min_r_m + e.band.max_r_m));
        let d = (ground - cam_local).length();
        let reach = patch_edge_arc_m(id.depth, radius_m);
        if d + reach >= hide_m as f64 && d - reach <= far_m as f64 {
            t.cards_in_window += e.card_trees;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::far_trees::fixtures::forest_def;
    use crate::planet_chunks::{
        select_patches, ChunkParams, PatchId, RadialBand, CHUNK_SPLIT_PX, MAX_BUILD_REQUESTS,
        MAX_CHUNK_LEAVES, MAX_PATCH_DEPTH,
    };

    /// The base-only selection as the frame loop builds it, floor included.
    fn base_only(floor: CardFloor, radius_m: f64) -> ChunkParams {
        ChunkParams {
            occluder_r_m: None,
            deep: Default::default(),
            card_floor: floor,
            radius_m,
            band: RadialBand { min_r_m: radius_m - 200.0, max_r_m: radius_m + 200.0 },
            max_depth: MAX_PATCH_DEPTH,
            split_px: CHUNK_SPLIT_PX,
            px_per_rad: 1080.0 / 60f32.to_radians(),
            max_leaves: MAX_CHUNK_LEAVES,
            max_build_requests: MAX_BUILD_REQUESTS,
        }
    }

    /// THE BUG (BUG-301): a game without the tile tier draws its ground to depth 13 and the card
    /// stage needs 15, so it had no card stage at all. With the floor the frame loop asks for at
    /// the shipped card distance (1,500 m), a camera 40 m over a forest draws card-bearing ground
    /// out to that distance, and no deeper than the card depth.
    ///
    /// Seen red 2026-11-14 before `max_depth_for` applied the floor: "a base-only game drew no
    /// depth-15 ground, so it has no tree card stage; deepest drawn 13, card reach 0 m".
    #[test]
    fn a_game_without_the_tile_tier_still_has_a_card_stage() {
        let def = forest_def();
        let floor = CardFloor::for_stage(true, 0.6, 1_500.0);
        assert_eq!(floor, CardFloor { depth: TREE_MIN_DEPTH, within_m: 1_500.0 });
        let params = base_only(floor, def.radius);
        let measured = RadialBand { min_r_m: def.radius - 2.0, max_r_m: def.radius + 2.0 };
        let cam = DVec3::new(def.radius + 40.0, 0.0, 0.0);
        let sel = select_patches(cam, None, &|_| Some(measured), &params);
        let deepest = sel.draws.iter().map(|d| d.depth).max().unwrap_or(0);
        assert!(
            sel.draws.iter().any(|d| d.depth >= TREE_MIN_DEPTH),
            "a base-only game drew no depth-{TREE_MIN_DEPTH} ground, so it has no tree card stage; \
             deepest drawn {deepest}, card reach {:.0} m",
            sel.stats.card_reach_m
        );
        assert!(
            sel.stats.card_reach_m >= 1_400.0,
            "the card ground ends at {:.0} m, short of the 1,500 m card distance",
            sel.stats.card_reach_m
        );
        assert!(deepest <= TREE_MIN_DEPTH, "the floor refined past the card depth: {deepest}");
        // Past the card distance the cap is the planet's own: no patch deeper than 13 starts
        // beyond it (the floor costs nothing far away).
        for id in &sel.draws {
            if id.depth > MAX_PATCH_DEPTH {
                let b = crate::planet_chunks::patch_bounds(
                    &crate::planet_chunks::patch_corners(id),
                    def.radius,
                    &measured,
                );
                let nearest = ((cam - b.bound_center).length() - b.bound_radius).max(0.0);
                assert!(
                    nearest <= 1_500.0 + patch_edge_arc_m(id.depth - 1, def.radius),
                    "a depth-{} patch {nearest:.0} m away, far past the card distance",
                    id.depth
                );
            }
        }
    }

    /// The floor is off where nothing could stand on it, and it never lowers a cap.
    #[test]
    fn the_floor_is_off_where_no_tree_can_stand_and_never_lowers_a_cap() {
        assert_eq!(CardFloor::for_stage(false, 0.6, 1_500.0), CardFloor::default());
        assert_eq!(CardFloor::for_stage(true, 0.0, 1_500.0), CardFloor::default());
        assert_eq!(CardFloor::for_stage(true, f32::NAN, 1_500.0), CardFloor::default());
        let f = CardFloor::for_stage(true, 0.6, 1_500.0);
        assert_eq!(f.cap(MAX_PATCH_DEPTH, 10.0), TREE_MIN_DEPTH);
        assert_eq!(f.cap(MAX_PATCH_DEPTH, 1_500.0), TREE_MIN_DEPTH);
        assert_eq!(f.cap(MAX_PATCH_DEPTH, 1_501.0), MAX_PATCH_DEPTH);
        assert_eq!(f.cap(20, 10.0), 20, "the tile tier's deeper cap is kept");
        assert_eq!(CardFloor::default().cap(MAX_PATCH_DEPTH, 0.0), MAX_PATCH_DEPTH);
    }

    /// The counter counts what the draw list carries: card patches, their trees, and the trees in
    /// patches overlapping the kept window. A draw list of base-only ground reads zero cards,
    /// which is the reading that proved BUG-301.
    #[test]
    fn the_tally_counts_the_cards_the_draw_list_carries() {
        let r = 6_371_000.0;
        let mut cs = ChunkState::new(1);
        let band = RadialBand { min_r_m: r - 10.0, max_r_m: r + 10.0 };
        let cam = DVec3::new(r + 40.0, 0.0, 0.0);
        let id = |d: u8, i: u32| {
            let mut p = PatchId::root(0);
            for _ in 0..d {
                p = p.child(i);
            }
            p
        };
        // Base-only ground: a depth-13 patch carries no card.
        let shallow = id(MAX_PATCH_DEPTH, 1);
        cs.insert_built(shallow, 0, None, 1, DVec3::new(r, 0.0, 0.0), band, 0.0, 0);
        cs.last_drawn.insert(shallow);
        let t = tally_from(&cs, cam, r, 0.0, 1_500.0);
        assert_eq!((t.drawn_patches, t.card_patches, t.cards, t.max_drawn_depth), (1, 0, 0, MAX_PATCH_DEPTH));
        // A near card patch inside the window and a far one past the cutoff.
        let near = id(TREE_MIN_DEPTH, 2);
        let far = id(TREE_MIN_DEPTH, 3);
        cs.insert_built(near, 1, None, 1, DVec3::new(r, 300.0, 0.0), band, 0.6, 40);
        cs.insert_built(far, 2, None, 1, DVec3::new(r, 5_000.0, 0.0), band, 0.6, 25);
        cs.last_drawn.insert(near);
        cs.last_drawn.insert(far);
        let t = tally_from(&cs, cam, r, 0.0, 1_500.0);
        assert_eq!((t.drawn_patches, t.card_patches, t.cards, t.cards_in_window), (3, 2, 65, 40));
        assert_eq!(t.max_drawn_depth, TREE_MIN_DEPTH);
        // A hide radius past the near patch leaves it out of the window too.
        let t = tally_from(&cs, cam, r, 900.0, 1_500.0);
        assert_eq!(t.cards_in_window, 0);

        // HIGH GROUND (Fuji, 1.3 km up): the anchor sits on the sea-level sphere, the ground and
        // its cards 1,300 m above it. From 350 m over that ground a patch 1 km off is well inside
        // the 1,500 m window. Seen red 2026-11-14 measuring from the anchor: "a card patch on high
        // ground counted out of the window" (the first boot read 0 in the window at
        // fuji-forest-hillside over a full forest).
        let mut hill = ChunkState::new(1);
        let high = RadialBand { min_r_m: r + 1_290.0, max_r_m: r + 1_310.0 };
        let patch = id(TREE_MIN_DEPTH, 1);
        hill.insert_built(patch, 0, None, 1, DVec3::new(r, 1_000.0, 0.0), high, 0.6, 30);
        hill.last_drawn.insert(patch);
        let over = DVec3::new(r + 1_650.0, 0.0, 0.0);
        let t = tally_from(&hill, over, r, 0.0, 1_500.0);
        assert_eq!(t.cards_in_window, 30, "a card patch on high ground counted out of the window");
    }
}
