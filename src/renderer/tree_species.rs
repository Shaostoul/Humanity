//! The SPECIES ARCHITECTURE: how each `form` lays out a crown (v0.1102).
//!
//! Split out of `tree_mesh.rs` at exactly the line the file-size ratchet
//! (`tests/file_size_ratchet.rs`) names as the intended extraction: the
//! conifer / umbrella / palm / broadleaf crown builders and the recursion
//! they share, leaving `tree_mesh` as the geometry kernel - tubes, rings,
//! junctions, welds, flare, cluster cards - plus its gates.
//!
//! It is a CHILD module of `tree_mesh` rather than a sibling, and that is
//! not a style accident: a crown builder touches almost every private item
//! of the kernel (`TreeParts`, `Junction`, `LimbShape`, `Foliage`, `Twig`,
//! the vector helpers, the radius laws), and a child sees all of them
//! through one `use super::*`. As a sibling, every one of those would need
//! widening to `pub(crate)` - a much larger and more permanent change to the
//! kernel's surface than the split is worth.
//!
//! WHAT EVERY FORM IN HERE OWES THE KERNEL: one `Twig` per foliage-bearing
//! limb, pushed into the `twigs` vector it is handed. That vector is the
//! contract cluster cards are planned from, and three of the four forms
//! quietly failed to keep it until v0.1102 - see
//! `tree_mesh::tests::every_form_emits_cards_when_given_a_cluster_block`.

use super::*;

/// Recursive limb. Emits a tapered segment, then either children or foliage.
///
/// `parent` describes the surface this limb grows out of; the limb's spine
/// starts ON that surface (`surface_root`) and `TreeParts::weld_child` skins
/// the collar that closes the join. Nothing is buried - see the block comment
/// above `FORK_GATE_RINGS` for why the burial had to go.
#[allow(clippy::too_many_arguments)]
fn limb(
    b: &mut TreeParts,
    def: &TreeDef,
    parent: Junction,
    dir: [f32; 3],
    len: f32,
    r0: f32,
    depth: u32,
    max_depth: u32,
    fol: Foliage,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
    if b.tri_count() > MAX_TRIS || len < 0.05 {
        return;
    }
    // Taper toward a REAL twig, not by a flat ratio - see `limb_tip_radius`.
    let r1 = limb_tip_radius(r0, depth, max_depth);
    // A limb is a CURVED SPINE, not one straight frustum (v0.1067). Real
    // branches bow: they leave the parent at an angle, then gravity pulls the
    // far end down while the tip reaches back toward the light. Drawing that as
    // a single cone gave every junction a hard kink and every branch a dead
    // straight silhouette, which is most of what read as "early 2000s".
    let segs = segments_for(depth);
    let sides = sides_for(depth);
    let shape = LimbShape::new(parent, dir, len, r0, r1, sides);
    // The join: spine start ON the parent's surface, collar skinned across.
    // `reps` is this limb's ONE bark repeat count, opened by the weld and used
    // by every segment below, root to tip (v0.1100).
    let (start, reps) = b.weld_child(parent, shape, def.trunk_color);
    let mut p = start;
    let mut d = dir;
    // Taper measured from the root ring, which is now the first VISIBLE ring:
    // with nothing buried the two are the same thing.
    let taper = |x: f32| (x / len.max(1e-4)).clamp(0.0, 1.0);
    let mut tube_far = start;
    let mut tube_dir = d;
    let mut tube_r = r0;
    // The flare at the LAST ring drawn, so the end cap closes exactly the
    // ellipse the tube ended on (v0.1100).
    let mut tube_flare = shape.at(0.0);
    // Ring stations: the uniform ones the bow is sampled at, PLUS rings packed
    // into the base flare so the profile is drawn rather than stepped over.
    let stations = ring_stations(len, r0, segs, b.flare_min_r);
    b.flare_tris += (stations.len() - 1).saturating_sub(segs as usize) * sides as usize * 2;
    let nominal = len / segs as f32;
    for w in 0..stations.len() - 1 {
        let (xa, xb) = (stations[w], stations[w + 1]);
        let seg_len = xb - xa;
        let to = add(p, d, seg_len);
        // PLAIN taper radii; the directional flare rides on top of them per
        // vertex inside `bark_tube`, at the PROFILE stations `xa`/`xb` rather
        // than at the overshot geometry (v0.1100).
        let (ra, rb) = (shape.base_radius_at(xa), shape.base_radius_at(xb));
        // Overshoot the joint by a fraction of the radius so the kink between
        // two spine segments cannot open a slit on the outside of the bow. The
        // spine itself still advances by exactly `seg_len`.
        //
        // CAPPED AT HALF THE SEGMENT (v0.1099): the flare's rings are a
        // fraction of a radius apart, and a fixed 0.7-radius overshoot there
        // would draw each little frustum five times past its own end - which
        // stretches the flare's radius fall over the wrong distance and lands
        // the next ring inside the previous tube at a different width. Half a
        // segment still overlaps far more than the sub-degree kink needs.
        // v runs from the root ring, so a bowing limb's bark is one continuous
        // run across its spine joints (v0.1089).
        tube_far = add(p, d, seg_len + (rb * 0.7).min(seg_len * 0.5));
        tube_dir = d;
        tube_r = rb;
        tube_flare = shape.at(xb);
        let seg = BarkSeg { reps, v0_m: xa, near: shape.at(xa), far: shape.at(xb) };
        b.bark_tube(p, tube_far, ra, rb, sides, def.trunk_color, seg);
        // The gate reads the leading rings of every limb straight off the
        // geometry that was drawn, so record them here and nowhere else.
        if w == 0 {
            b.note_fork_ring(p, d, ra, sides, shape.at(xa));
        }
        b.note_fork_ring(tube_far, d, rb, sides, shape.at(xb));
        p = to;
        // Bow: droop grows toward the tip, and the trunk stays straighter than
        // the twigs (a bole that sagged would read as a sick tree). Scaled by
        // the step's share of a nominal segment so that adding flare rings
        // near the root cannot bend the limb any further than it used to.
        let droop = if depth == 0 { 0.04 } else { 0.10 } * taper(xb) * (seg_len / nominal);
        d = norm([d[0], d[1] - droop, d[2]]);
    }
    let to = p;

    // Foliage on the outer TWO generations, not just the tips: one generation
    // of leaf clumps leaves a crown you can see straight through. And on ANY
    // limb already tapered to a shoot, whatever generation it happens to be -
    // see `FOLIAGE_TIP_R_M` for why a generation number stopped being a usable
    // rule once laterals started carrying their own `max_depth`.
    if depth + 1 >= max_depth || r1 <= FOLIAGE_TIP_R_M {
        let blossom = def.blossom_frac > 0.0 && rng.range(0.0, 1.0) < def.blossom_frac;
        let color = if blossom { def.blossom_color } else { def.leaf_color };
        let tip = depth >= max_depth;
        let at = if tip { to } else { add(start, dir, len * 0.72) };
        let f = if tip { fol } else { fol.with_clump(0.78) };
        // The twig sleeve cluster cards ride on (v0.1088). Recorded here, not
        // re-derived later, because "the outer two generations" is exactly the
        // set that carries foliage and the two must never disagree. `from` is
        // the limb's ROOT RING (v0.1098) - the sleeve covers drawn wood, and
        // drawn wood now begins on the parent's surface rather than inside it.
        twigs.push(Twig { from: start, dir, end: to, len, tip, tube_end: tube_far, tip_r: tube_r });
        // A clustered species keeps a much thinner blade layer: the cards are
        // the canopy now, and blades only earn their triangles inside ~2 m.
        // v0.1090 cuts it again, and pulls it IN: 6% of sakura's blade faces
        // were standing proud of the card mass on their own twig, which at
        // range is a fringe of raw triangles round the silhouette of every
        // blossom cluster.
        let (sprigs, f) = match def.clusters.as_ref() {
            Some(cd) => (if tip { 3 } else { 2 }, f.with_clump(near_blade_clump_k(cd, f))),
            None => (if tip { 16 } else { 8 }, f),
        };
        leaf_cluster(&mut b.foliage, at, dir, f, color, sprigs, rng);
        if tip {
            return;
        }
    }

    // Children, fanned by the golden angle so they do not stack.
    //
    // A 3-WAY FORK ONLY AT THE LATERAL'S OWN BASE (v0.1096). Real branch
    // architecture is overwhelmingly bifurcating; a 45% chance of three
    // children at EVERY generation, which is what this was, multiplies the
    // twig count by 2.45 per level instead of 2. That was affordable while a
    // broadleaf had three limbs planned to a fixed depth of 3; it is not once
    // the crown has six scaffold limbs shed up a leader, because the twig
    // count is what the cluster-card planner spends and overrunning
    // CARD_TRI_BUDGET makes the stretch backstop thin every sleeve to one
    // station. Keeping the trefoil at the base is where it reads anyway - the
    // first fork of a big limb is the one you see from the ground.
    let coin = rng.range(0.0, 1.0);
    let n = if depth == 0 && coin < 0.45 { 3 } else { 2 };
    // This limb is about to become a fork, so close its open end ONCE - not
    // once per child, which would stack coplanar discs and z-fight. The hole
    // this closes is the same hole the v0.1086 burial used to plug from the
    // inside.
    b.bark_cap(tube_far, tube_dir, tube_r, sides, def.trunk_color, tube_flare);
    // The surface every child of this limb welds onto: the far end of the last
    // drawn ring, facing the way that ring faced. Its radius is the PLAIN
    // taper, which is what the drawn ring is to within the flare's residual -
    // under 1% of the shaft this far out, because a limb is always many shaft
    // radii long (`FLARE_SPAN` is 3) - and the plain radius is the one a child
    // can project its collar feet onto as a cylinder.
    let fork = Junction { axis: tube_far, up: tube_dir, r: tube_r, tip: true };
    // WOOD IS CONSERVED ACROSS A FORK (v0.1099), not duplicated. Through
    // v0.1098 each child started at exactly `r1`, so a two-way fork carried
    // TWICE the parent's cross-sectional area out of it and a three-way three
    // times - which is the "gets very wide shortly after" half of the operator's
    // report, and it compounds every generation. The pipe model splits it:
    // 0.74 of `r1` for two children, 0.62 for three. The directional flare then
    // brings the crotch side of the WELD back to 0.93 (two-way) or 0.78
    // (three-way) of the parent's tip, so the fork reads as continuous wood
    // that never stands proud of what feeds it.
    let child_r0 = fork_child_radius(r1, n);
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.5, 0.5) + depth as f32;
        let spread = rng.range(22.0, 42.0) - if depth == 0 { 6.0 } else { 0.0 };
        let d = tilt(dir, spread, phase);
        // Phototropism: every generation bends back toward vertical, which is
        // what rounds a crown instead of leaving it a flat fan.
        let d = norm([d[0], d[1] + 0.22, d[2]]);
        // Twigs shorten faster than limbs, so foliage clumps sit close together
        // instead of leaving long bare runs between them.
        let child_len = len * if depth + 2 >= max_depth { rng.range(0.42, 0.56) } else { rng.range(0.62, 0.78) };
        limb(b, def, fork, d, child_len, child_r0, depth + 1, max_depth, fol.with_clump(0.86), rng, twigs);
    }
}

/// A DECURRENT BROADLEAF: a clear bole, then a continuing leader that sheds
/// laterals at stations up through the crown zone (v0.1096).
///
/// WHAT THIS REPLACES AND WHY. Through v0.1095 this function put a short bole
/// down and then spawned all three primaries from ONE point - the bole top -
/// with every limb planned to the same `max_depth = 3` and foliage emitted only
/// on generations 2 and 3. Traced at species means for an 8 m sakura that put
/// every foliage-bearing twig end between y 6.3 and 6.8 m, so the whole crown
/// occupied roughly y 5.6-7.6: a live crown ratio of 0.25 against a 7 m spread,
/// aspect 3.5:1. On screen that is a mushroom cap with a STRAIGHT-CUT
/// underside, every cap in the stand bottoms out at the same world height, and
/// below that line the frame is bare trunks you can see the horizon through for
/// hundreds of metres. It reads as a planted orchard, not a forest.
///
/// REAL BEHAVIOUR. Live crown ratio (crown length over total height) is
/// 0.35-0.60 for forest-grown dominant and codominant temperate broadleaves
/// (forestry management targets >= 0.40 for oak), and an OPEN-GROWN
/// Prunus x yedoensis carries foliage from a 1.5-2.5 m crown base to the apex,
/// i.e. 0.7-0.8. That depth comes from the vertical distribution of branch
/// ORIGINS along the stem - laterals shed over 1-3 m of it, the older and lower
/// ones longer and inserted closer to horizontal - not from one whorl. It is
/// also why a real crown's lower boundary is a ragged lobed sheath rather than
/// a horizontal cut, and why you cannot see through a cherry orchard at eye
/// level.
///
/// SO: one leader to ~0.85 H, `LATERALS` shed at stations spanning the crown
/// zone, length falling linearly with station height, insertion angle rising
/// toward vertical with height, and one terminal limb continuing the leader.
/// Each lateral gets its OWN generation count from its OWN length
/// (`generations_for`), which is what stops a 1.2 m upper lateral costing the
/// same subtree as a 3 m lower one. Foliage is keyed on shoot radius rather
/// than on a generation number (`FOLIAGE_TIP_R_M`), because with per-lateral
/// depths a generation number no longer describes anything physical.
///
/// The LAI fit needs no change: `emit_cluster_cards` solves the card side
/// against `crown_of`'s envelope, so a deeper crown simply gets more cards at
/// the same target.
pub(super) fn broadleaf(
    b: &mut TreeParts,
    def: &TreeDef,
    h: f32,
    density: f32,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
    // Crown base: where the lowest lateral leaves the stem. Cherry and maple
    // branch low; a forest oak higher. Kept at the low end of the v0.1095 band
    // because live crown ratio is measured from HERE.
    let bole_frac = rng.range(0.24, 0.32);
    // Where the leader stops and hands over to its terminal limb. A decurrent
    // broadleaf loses apical dominance in the crown, so the leader thins hard
    // and the topmost laterals close over it.
    let leader_frac = rng.range(0.82, 0.88);
    let r_base = h * 0.030;
    let lean = norm([rng.range(-0.06, 0.06), 1.0, rng.range(-0.06, 0.06)]);
    let stem_len = h * leader_frac;
    // r_top_frac 0.26: chosen so the stem still reads 0.74 of its base radius
    // at the OLD bole top (~0.36 of this stem's length), i.e. the part of the
    // trunk you stand next to is unchanged and only the new upper run is new.
    // 10 segments rather than 6 because the stem is about three times longer.
    let (stem, stem_end) = trunk(b, def, [0.0, 0.0, 0.0], lean, stem_len, r_base, 0.26, h * 0.09, 10);
    // Clumps stay at ~10% of tree height (the clump SHAPE is unchanged), but a
    // clump is filled with real leaves instead of being one. Leaf length rides
    // on the species height so an oak leaf is bigger than a maple leaf without
    // inventing a data field, and it sits at the TOP of the real range for
    // each: an oak leaf is 0.10-0.22 m and a big-leaf oak more, so 0.20 m is
    // honest, and every square centimetre of honest leaf is canopy coverage
    // that costs no extra triangle. The 0.09 m floor is where cherry and maple
    // land, and they are genuinely that small.
    let fol = foliage_of(def, h, density);

    // 6 shed laterals plus the leader's own terminal limb = 7 scaffold limbs,
    // against the 3 of v0.1095. Not more: the cost of a crown is its TWIG
    // count (that is what the card planner spends), and 7 limbs at the depths
    // `generations_for` picks lands ~30% above the v0.1095 twig count, which
    // the card budget absorbs without the stretch backstop biting.
    const LATERALS: u32 = 6;
    // Crown zone as a fraction OF THE STEM (not of the tree): the lowest
    // lateral sits at the crown base, the highest just under the leader top.
    let zone_lo = (bole_frac / leader_frac).clamp(0.05, 0.9);
    let crown_len = h - h * bole_frac;
    for k in 0..=LATERALS {
        // u = 0 at the crown base, 1 at the leader top.
        let u = k as f32 / LATERALS as f32;
        let terminal = k == LATERALS;
        let sf = zone_lo + (1.0 - zone_lo) * u;
        let s = stem_at(&stem, sf);
        // LENGTH falls linearly with station height: the oldest, lowest
        // lateral is the longest, which is what a decurrent crown looks like
        // and what makes its lower boundary lobed instead of level. The
        // coefficient is LOWER than the v0.1095 primaries' 0.40-0.52 on
        // purpose: the crown zone is three times as tall now, so the crown
        // fills by being deep rather than by every limb reaching further. At
        // 0.44-0.56 the measured spread came out 5.24 m of RADIUS on an 8 m
        // sakura - a 10.5 m crown, back to a 2:1 plate, just a bigger one.
        let len = crown_len * rng.range(0.34, 0.44) * (1.0 - 0.55 * u);
        // INSERTION ANGLE rises toward vertical with height: wide at the crown
        // base, a steep fork at the top.
        //
        // NOT as wide as the anatomy alone would suggest, and the reason is
        // the bare inner run. Foliage rides on the outer generations, so a
        // lateral's FIRST generation is bare wood by construction - about 1.5-2
        // m of it. Through v0.1095 that run was hidden because all three
        // primaries pointed UP at 26-46 deg and sat inside the crown mass; the
        // first cut of this function put laterals out at 52-62 deg and the
        // probe capture came back with long bare arms silhouetted against sky
        // on every tree. 44-56 keeps the crown decurrent while tucking the bare
        // run back inside the foliage shell of its neighbours.
        let spread_deg = if terminal { rng.range(8.0, 18.0) } else { rng.range(44.0, 56.0) - 26.0 * u };
        // Golden angle between successive stations - real phyllotaxis, and it
        // stops two laterals stacking in one azimuth.
        let phase = k as f32 * 2.399_963 + rng.range(-0.35, 0.35);
        let d = tilt(s.dir, spread_deg, phase);
        // Phototropism, same as inside `limb`: every lateral bends back toward
        // the light, which rounds the crown instead of leaving a flat fan.
        let d = norm([d[0], d[1] + 0.10, d[2]]);
        // A lateral is thinner than the stem it leaves; the terminal limb
        // inherits the leader's tip radius outright.
        let r0 = if terminal { s.r * 0.92 } else { s.r * rng.range(0.52, 0.68) };
        let llen = len.max(0.3);
        // WHERE THE ROOT RING GOES (v0.1098). A shed lateral leaves the FLANK
        // of a stem that keeps going, so its junction is a side junction and
        // its spine starts on the bark at that height. The terminal limb takes
        // over from the leader, so its junction is the leader's END PLANE - and
        // that plane gets a real cap rather than being plugged by burying the
        // limb through it. `stem_end` is the DRAWN end, past the last spine
        // point by the joint overshoot.
        let j = if terminal {
            Junction { axis: stem_end, up: s.dir, r: s.r, tip: true }
        } else {
            Junction { axis: s.p, up: s.dir, r: s.r, tip: false }
        };
        if terminal {
            // The stem has no directional flare (its root flare is radial, and
            // it has decayed to nothing this far up), so the cap is a circle.
            b.bark_cap(stem_end, s.dir, s.r, 8, def.trunk_color, None);
        }
        limb(b, def, j, d, llen, r0, 0, generations_for(llen), fol, rng, twigs);
    }
}

pub(super) fn conifer(
    b: &mut TreeParts,
    def: &TreeDef,
    h: f32,
    density: f32,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
    // A single straight leader with whorls of short, steeply drooping branches
    // that shorten toward the top: the classic conical silhouette.
    let r_base = h * 0.022;
    let top = [0.0, h, 0.0];
    // Leader radius at height fraction `f`, so a whorl branch can size itself
    // against the trunk it leaves instead of against the trunk BASE. With the
    // old flat `r_base * 0.22`, the topmost whorl's root ring (0.22 r_base) was
    // FATTER than the leader there (0.194 r_base) and stuck out through it.
    let leader_r = |f: f32| r_base * (1.0 + (0.16 - 1.0) * f);
    // The leader and its apex cone are ONE limb, so they share one bark repeat
    // count taken from the butt (v0.1100) - the apex used to round to its own
    // and step the plate scale right where the two meet.
    let leader_reps = b.open_bark_run(r_base);
    b.bark_tube(
        [0.0, 0.0, 0.0],
        top,
        r_base,
        r_base * 0.16,
        8,
        def.trunk_color,
        BarkSeg { reps: leader_reps, v0_m: 0.0, near: None, far: None },
    );
    // Close the leader's open apex ring with a short cone (16 triangles). It is
    // the one hole on a conifer you look straight down into from the air. Its
    // bark v continues from the leader's top so the pattern does not restart.
    b.bark_tube(
        top,
        [0.0, h + h * 0.012, 0.0],
        r_base * 0.16,
        0.0,
        8,
        def.trunk_color,
        BarkSeg { reps: leader_reps, v0_m: h, near: None, far: None },
    );
    let whorls = 9;
    // A conifer's drawn element is a NEEDLE SPRAY, not a single needle: one
    // 30 mm needle would be far below a pixel and there is no budget for the
    // ~200k of them a real fir carries. 0.30-0.50 m of narrow strap is the
    // honest unit, and it is 3x smaller than the 1.2 m kite it replaces.
    let fol = foliage_of(def, h, density);
    for w in 0..whorls {
        let f = 0.22 + 0.74 * (w as f32 / (whorls - 1) as f32);
        let y = h * f;
        let lr = leader_r(f);
        // Branch length tapers linearly to the apex.
        let blen = h * 0.30 * (1.0 - f) + h * 0.03;
        let per = 5;
        for k in 0..per {
            let phase = (w * per + k) as f32 * 2.399_963;
            let d = tilt([0.0, 1.0, 0.0], rng.range(74.0, 96.0), phase);
            let d = norm([d[0], d[1] - 0.30, d[2]]);
            // Rooted ON THE BARK of the leader, not on its axis (v0.1098). The
            // old axis rooting never poked out the back - a whorl branch is
            // thin and nearly perpendicular - but it ran a quarter-metre of
            // wood through the inside of the trunk on every one of the 45
            // branches, and the collar closes the join without it.
            let j = Junction { axis: [0.0, y, 0.0], up: [0.0, 1.0, 0.0], r: lr, tip: false };
            // One flared run (v0.1099): a whorl branch leaves a fir's leader
            // with the same swollen collar every other junction has. The
            // whorl's radius fractions are left alone - a conifer sheds ~45
            // branches off ONE continuous leader rather than forking, so the
            // pipe model has nothing to say here and 0.42 of the local leader
            // is already a measured-looking stub.
            let tip_r = lr * 0.14;
            let (root, tip) =
                b.flared_run(j, LimbShape::new(j, d, blen, lr * 0.42, tip_r, 4), def.trunk_color);
            // THE TWIG A WHORL BRANCH IS (v0.1102). Same contract `limb` fills,
            // field for field, because the card planner must not be able to
            // tell the forms apart: `from` is the ROOT RING on the leader's
            // bark (drawn wood starts there, so the sleeve covers drawn wood),
            // `dir` is the branch axis, and `end` is the far end of the last
            // ring `flared_run` drew.
            //
            // `end`, `tube_end` and `len` agree exactly here where they do not
            // in `limb`: a `flared_run` limb is STRAIGHT (no bowed spine) and
            // its stations end ON `len` with no joint overshoot, so the spine
            // end and the last drawn ring are the same point. `tip` is true
            // because nothing caps that ring - a whorl branch really does end
            // as an open pipe, which is precisely what a cap card is for.
            twigs.push(Twig {
                from: root,
                dir: d,
                end: tip,
                len: blen,
                tip: true,
                tube_end: tip,
                tip_r,
            });
            // THE BLADE LAYER SHRINKS WHEN CARDS CARRY THE CANOPY (v0.1102),
            // exactly as it does inside `limb`: fewer sprigs, and the clump
            // pulled inside the card mass by `near_blade_clump_k`. Without this
            // a carded fir scatters blades to one clump radius (1.10 m on a
            // 22 m tree) around a twig whose card shell ends at 0.82 m, and
            // every one of them is a raw triangle standing proud of the crown -
            // which is `near_blades_stay_inside_the_card_shell`'s whole subject.
            let (tip_sprigs, mid_sprigs, fol) = match def.clusters.as_ref() {
                Some(cd) => (3, 2, fol.with_clump(near_blade_clump_k(cd, fol))),
                None => (10, 7, fol),
            };
            leaf_cluster(&mut b.foliage, tip, d, fol, def.leaf_color, tip_sprigs, rng);
            // A second clump midway keeps the branch from reading as a bare stick.
            let mid = add(root, d, blen * 0.55);
            leaf_cluster(&mut b.foliage, mid, d, fol.with_clump(0.8), def.leaf_color, mid_sprigs, rng);
        }
    }
}

pub(super) fn umbrella(
    b: &mut TreeParts,
    def: &TreeDef,
    h: f32,
    density: f32,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
    // Acacia: a tall bare bole, then limbs that flatten hard into a wide,
    // level crown. The giveaway is that the crown is WIDER than the tree is
    // tall and its underside is flat.
    let bole = h * rng.range(0.52, 0.62);
    let r_base = h * 0.034;
    let top = [0.0, bole, 0.0];
    let bole_reps = b.open_bark_run(r_base);
    b.bark_tube(
        [0.0, 0.0, 0.0],
        top,
        r_base,
        r_base * 0.66,
        8,
        def.trunk_color,
        BarkSeg { reps: bole_reps, v0_m: 0.0, near: None, far: None },
    );
    let n = 5;
    // An acacia leaf is bipinnate: what reads at any real distance is a pinna,
    // a 0.10-0.18 m feather of leaflets, not a metre-wide blade. Acacia was the
    // most under-spent form in the tree budget (13% of MAX_TRIS), so it can
    // afford by far the densest sprigs, which is also what its flat crown wants.
    let fol = foliage_of(def, h, density);
    // The bole ends at `top` and every primary forks off that plane, so close
    // it once (v0.1098). Through v0.1097 the hole was plugged by burying the
    // primaries 1.4 radii back down the bole - which, at the 58-74 deg
    // insertion an acacia uses, put the buried ring a full radius sideways and
    // straight out through the far side of the trunk.
    let bole_r = r_base * 0.66;
    b.bark_cap(top, [0.0, 1.0, 0.0], bole_r, 8, def.trunk_color, None);
    let bole = Junction { axis: top, up: [0.0, 1.0, 0.0], r: bole_r, tip: true };
    // WOOD IS CONSERVED ACROSS THIS FORK (v0.1099), and this is the form where
    // getting it wrong was loudest - the operator's field report is an acacia.
    //
    // Through v0.1098 each of the FIVE primaries left the bole at exactly the
    // bole's own tip radius: five limbs each as thick as the trunk they came
    // out of, i.e. five times the cross-sectional area of the wood feeding
    // them. On a 9 m acacia that is a 0.40 m bole splitting into five 0.40 m
    // limbs, and no flare, collar or poly count can make that read as a tree -
    // it reads as a pipe manifold, which is exactly "gets very wide shortly
    // after". The pipe model gives 5^(-1/2.3) = 0.50, so a primary's SHAFT is
    // 0.20 m across where it was 0.40, and its flared weld is 0.29 m - visibly
    // narrower than the bole it leaves, which is what a real umbrella crown
    // looks like from underneath. The tip/base ratios of the primary (0.52) and
    // the fan (0.35) are unchanged, so the limbs still taper as they did.
    let primary_tip_frac = 0.52;
    let fan_tip_frac = 0.35;
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        // Steeply out, barely up.
        let d = tilt([0.0, 1.0, 0.0], rng.range(58.0, 74.0), phase);
        let seg = h * rng.range(0.30, 0.40);
        let p_r0 = fork_child_radius(bole_r, n);
        let p_r1 = p_r0 * primary_tip_frac;
        let p_shape = LimbShape::new(bole, d, seg, p_r0, p_r1, 5);
        let (_, mid) = b.flared_run(bole, p_shape, def.trunk_color);
        // The crown layer: near-horizontal fans of foliage, forking off the
        // primary's own end plane. The cap closes the primary's LAST ring, so
        // it takes that ring's flare station (v0.1100).
        b.bark_cap(mid, d, p_r1, 5, def.trunk_color, p_shape.at(seg));
        let elbow = Junction { axis: mid, up: d, r: p_r1, tip: true };
        for j in 0..3 {
            let p2 = phase + j as f32 * 1.9;
            let d2 = tilt([0.0, 1.0, 0.0], rng.range(80.0, 94.0), p2);
            let flen = h * rng.range(0.16, 0.26);
            let f_r0 = fork_child_radius(p_r1, 3);
            let f_r1 = f_r0 * fan_tip_frac;
            let (root, tip) = b.flared_run(
                elbow,
                LimbShape::new(elbow, d2, flen, f_r0, f_r1, 4),
                def.trunk_color,
            );
            // THE TWIG A CROWN FAN IS (v0.1102), and the acacia blocker's first
            // half closed: the fan is the only limb on this form that carries
            // foliage, so it is the only one a card sleeve belongs on. The
            // primaries deliberately record nothing - they are bare structural
            // wood under the crown, and sleeving them would hang foliage where
            // an acacia's flat underside is the whole silhouette.
            //
            // `dir` is the FAN'S OWN AXIS, not the world-up the leaf clump is
            // fanned about: the sleeve wraps the wood, and cards rotate about
            // the axis they are given.
            twigs.push(Twig {
                from: root,
                dir: d2,
                end: tip,
                len: flen,
                tip: true,
                tube_end: tip,
                tip_r: f_r1,
            });
            // Same card-shell containment the conifer and `limb` apply: with
            // cards the blades are close-range detail that must live INSIDE
            // the card mass, and an acacia's clump is 0.72 m on a 9 m tree
            // against a card shell of ~0.82 m.
            let (sprigs, fol) = match def.clusters.as_ref() {
                Some(cd) => (4, fol.with_clump(near_blade_clump_k(cd, fol))),
                None => (16, fol),
            };
            leaf_cluster(&mut b.foliage, tip, [0.0, 1.0, 0.0], fol, def.leaf_color, sprigs, rng);
        }
    }
}

pub(super) fn palm(
    b: &mut TreeParts,
    def: &TreeDef,
    h: f32,
    density: f32,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
    // No branches at all and no secondary thickening: a palm is a single
    // unbranched stem with a crown of fronds at the top, and an old palm is
    // not a fatter palm. Modelled as a gently curved stack of segments.
    let segs = 7;
    let r_base = h * 0.028;
    let curve = rng.range(-0.10, 0.10);
    let mut p = [0.0f32, 0.0, 0.0];
    let mut d = norm([curve, 1.0, rng.range(-0.08, 0.08)]);
    // Running arc length: a palm stem is one continuous column of leaf scars,
    // so its bark v must not restart at each of the seven segments (v0.1089).
    let mut v = 0.0f32;
    // A palm stem is one limb, so one repeat count from its butt (v0.1100).
    // It barely tapers (0.70 of base at the crown), so the plates foreshorten
    // by well under half a cell over the whole 12 m column.
    let reps = b.open_bark_run(r_base);
    for i in 0..segs {
        let f = i as f32 / segs as f32;
        let seg = h / segs as f32;
        let to = add(p, d, seg);
        b.bark_tube(
            p,
            to,
            r_base * (1.0 - f * 0.35),
            r_base * (1.0 - (f + 0.15) * 0.35),
            7,
            def.trunk_color,
            BarkSeg { reps, v0_m: v, near: None, far: None },
        );
        p = to;
        v += seg;
        d = norm([d[0] + curve * 0.06, d[1], d[2]]);
    }
    // Cap the stem's open top ring; the crown hides it from the side but not
    // from above, and a palm is a thing you fly over.
    let cap_r = r_base * 0.65;
    b.bark_tube(
        p,
        add(p, d, cap_r * 0.8),
        cap_r,
        0.0,
        7,
        def.trunk_color,
        BarkSeg { reps, v0_m: v, near: None, far: None },
    );
    // Crown: long fronds arching out and down. A frond is PINNATE - a rachis
    // carrying two ranks of strap leaflets - not one solid blade. The old code
    // drew each of 15 fronds as a single `b.leaf`, i.e. a 4 m x 1 m sheet, and
    // that is the giant-kite defect in its purest form. Palm was also the most
    // under-spent form in the whole generator at 4% of MAX_TRIS.
    let frond = h * 0.34;
    for k in 0..28 {
        let phase = k as f32 * 2.399_963;
        let dd = tilt([0.0, 1.0, 0.0], rng.range(46.0, 92.0), phase);
        let dd = norm([dd[0], dd[1] - 0.28, dd[2]]);
        // The frond stays on the FOLIAGE mesh, deliberately: its rachis carries
        // `leaf_color`, not `trunk_color`, so routing it onto the bark material
        // would paint 28 green rachises per palm bark-brown and tile a
        // trunk-scale bark texture onto a 3 cm stalk.
        let (end, tube_end, tip_r) =
            pinnate_frond(&mut b.foliage, p, dd, frond, density, def.leaf_color, rng);
        // THE TWIG A FROND IS (v0.1102). A palm carries no `clusters` block
        // today, and this is exactly why the record is made anyway: the twig
        // set is the CONTRACT between a form and the card planner, and a form
        // that keeps it only when someone remembers is the hole this arc
        // closes. The rachis ARCHES, so `end` (the last spine point) and
        // `tube_end` (the last drawn ring, one joint overshoot further) are
        // genuinely different points here, same as on a bowed `limb`.
        twigs.push(Twig { from: p, dir: dd, end, len: frond, tip: true, tube_end, tip_r });
    }
}

/// One palm frond: an arching rachis with two ranks of leaflets.
///
/// Leaflet length is ~20% of the frond, which on a 12 m palm is 0.8 m - the
/// real figure for a coconut pinna, and 5x smaller than the sheet it replaces.
///
/// Returns what a `Twig` needs off the rachis (v0.1102): the last spine point,
/// the far end of the last DRAWN ring (one joint overshoot beyond it), and that
/// ring's radius. Returned rather than re-derived by the caller because the
/// rachis arches - reconstructing its end outside this function would mean
/// copying the arch integration, and a copy that drifts describes wood that is
/// not there.
#[allow(clippy::too_many_arguments)]
fn pinnate_frond(
    b: &mut PlantMeshBuilder,
    base: [f32; 3],
    dir: [f32; 3],
    len: f32,
    density: f32,
    color: [f32; 3],
    rng: &mut Rng,
) -> ([f32; 3], [f32; 3], f32) {
    const SEGS: usize = 4;
    let mut pts = [[0.0f32; 3]; SEGS + 1];
    let mut q = base;
    let mut d = dir;
    pts[0] = q;
    let mut tube_far = base;
    let mut tube_r = 0.0f32;
    for s in 0..SEGS {
        let seg = len / SEGS as f32;
        let r = |f: f32| len * 0.018 * (1.0 - f) + len * 0.003;
        let (ra, rb) = (r(s as f32 / SEGS as f32), r((s + 1) as f32 / SEGS as f32));
        // Joint overshoot, same trick as the limb spine.
        tube_far = add(q, d, seg + rb * 0.8);
        tube_r = rb;
        b.tube(q, tube_far, ra, rb, 3, color);
        q = add(q, d, seg);
        pts[s + 1] = q;
        // The rachis arches over: a straight frond reads as a plank.
        d = norm([d[0], d[1] - 0.15, d[2]]);
    }
    let leaflet = len * 0.20;
    // Leaflet pairs are the palm's density knob. A real coconut frond carries
    // ~100 leaflets a side, so there is no honest ceiling short of the budget.
    let pairs = ((26.0 * density).round() as usize).clamp(6, 70);
    for i in 0..pairs {
        let f = (i as f32 + 0.5) / pairs as f32;
        let t = f * SEGS as f32;
        let si = (t as usize).min(SEGS - 1);
        let ft = t - si as f32;
        let a = pts[si];
        let bb = pts[si + 1];
        let node = [
            a[0] + (bb[0] - a[0]) * ft,
            a[1] + (bb[1] - a[1]) * ft,
            a[2] + (bb[2] - a[2]) * ft,
        ];
        let axis = norm([bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]]);
        let up = if axis[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let side = norm(cross(axis, up));
        // Leaflets shorten toward the tip and sweep forward along the rachis.
        let ll = leaflet * (1.0 - 0.45 * f) * rng.range(0.85, 1.12);
        for rank in [-1.0f32, 1.0] {
            let ld = norm([
                axis[0] * 0.42 + side[0] * rank * 0.9,
                axis[1] * 0.42 + side[1] * rank * 0.9 - 0.42,
                axis[2] * 0.42 + side[2] * rank * 0.9,
            ]);
            blade(b, node, ld, ll, 0.16, color, rng);
        }
    }
    (pts[SEGS], tube_far, tube_r)
}
