//! HOW THICK IS WOOD: the generator's radius and taper model (v0.1103).
//!
//! Split out of `tree_mesh.rs` for the same reason `tree_species.rs` was - the
//! kernel was over its file budget - and along a line that is a real seam
//! rather than a convenient one. Everything here answers ONE question in
//! METRES: given a tree of height H, or a limb of length L, or a fork of N
//! ways, how thick is the wood? The kernel next door answers "where do the
//! vertices go"; the species builders answer "what shape is the crown". Those
//! are three different jobs and they were tangled in one file.
//!
//! A CHILD module of `tree_mesh`, declared with `#[path]`, exactly as
//! `tree_species` is: it needs `TreeDef` and it is needed BY `tree_species`,
//! and a child sees the kernel's private items through one `use super::*`
//! while the kernel re-exports these through its own private `use`.
//!
//! ── WHY THIS MODULE EXISTS AT ALL (the v0.1103 field report) ─────────────
//!
//! The operator's most persistent complaint about the trees, across several
//! reports: branches "look like plumbing", junctions read as "a pipe elbow",
//! bases are "way too bulky". The junction collar was reshaped TWICE chasing
//! it (v0.1099 flare, v0.1100 directional collar). The collar was never the
//! problem. The RADII were, and they were wrong in metres:
//!
//!   - a 22 m fir was built with a 0.484 m trunk RADIUS - a 0.97 m bole, where
//!     a real 22 m Abies is 0.50-0.55 m through. 1.9x too fat.
//!   - an acacia's terminal twigs ended at 11.3 mm of radius, 23 mm across,
//!     against a real last-year shoot at 3-8 mm across. 4.5x too fat.
//!   - a fir's whorl-branch tips ended at 55 mm of radius, against a real
//!     branch tip at 4-8 mm. 13x too fat.
//!
//! None of the six radius gates could have caught any of it, because every one
//! of them is SCALE-FREE: the flare gate measures weld-over-shaft, the
//! back-poke gate measures child-against-parent, the taper gate measures
//! monotonicity. All three stay green when every radius in the tree is doubled.
//! `tree_radii_are_botanically_plausible` is the gate that reads metres, and it
//! is the one this module was written to satisfy.
//!
//! ── THE SOURCES ─────────────────────────────────────────────────────────
//!
//! Everything below is a published relation with its numbers, not a taste
//! call. The four that carry the model:
//!
//!  1. ELASTIC SIMILARITY. Greenhill (1881) gives the height at which a
//!     self-supporting column buckles under its own weight,
//!     `H_crit = C (E / rho g)^(1/3) D^(2/3)`; McMahon, "Size and Shape in
//!     Biology", Science 179 (1973), measured US record trees and found them
//!     clustered along that line at roughly a quarter of the critical height.
//!     A constant safety factor therefore means `D` scales as `H^(3/2)`, NOT
//!     as `H`. This is why a flat `radius = height * k` is wrong in principle
//!     as well as in value: it makes a sapling too fat or a giant too thin,
//!     and there is no k that is right at both ends.
//!  2. MEASURED EXPONENTS. O'Brien, Hubbell, Spiro, Condit & Foster, Ecology
//!     76 (1995), and King's work on tropical crown allometry, put the field
//!     exponent at `H ~ D^0.55..0.70`, i.e. `D ~ H^1.4..1.8`. Elastic
//!     similarity's 3/2 sits in the middle of that band, so it is used here.
//!  3. SLENDERNESS. Silviculture's standard stability metric is the h/d ratio,
//!     height over diameter at breast height in the SAME units. Wonn & O'Hara,
//!     "Height:diameter ratios and stability relationships for four northern
//!     Rocky Mountain tree species", West. J. Appl. For. 16 (2001), is the
//!     usual citation for the thresholds: above ~80 a stem is at high risk of
//!     snow and wind breakage, below ~60 it is stable. Dense-stand conifers run
//!     80-100, dominant open-grown conifers 40-60, open-grown temperate
//!     broadleaves 25-45, savanna trees lower still.
//!  4. THE PIPE MODEL. Shinozaki, Yoda, Hozumi & Kira, "A quantitative
//!     analysis of plant form - the pipe model theory", Jap. J. Ecol. 14
//!     (1964): the cross-sectional area of wood at any point is proportional
//!     to the leaf area it supports, so area is CONSERVED across a fork. Field
//!     measurements put the exponent at 2.0-2.5 rather than an exact 2,
//!     because a fork also has to be stiff. This was already in the file
//!     (`FORK_AREA_EXP`) and is unchanged; what changes is that laterals shed
//!     off a CONTINUING axis are no longer sized as if they were forks.
//!
//! Palms are the one form that follows none of this, and the reason is
//! anatomical rather than a special case: a palm is a monocot with no vascular
//! cambium, so it has no secondary thickening at all (Tomlinson, "The
//! Structural Biology of Palms", 1990). Its stem diameter is established at
//! the apical meristem before the internodes elongate, which means a TALLER
//! palm of the same species is not a THICKER one. Its diameter exponent is
//! therefore 0, not 3/2.

use super::*;

// ── STEM ALLOMETRY: how thick is a tree of height H? ─────────────────────

/// Stem slenderness `H/D` a growth FORM is built at when its species row does
/// not state one - dimensionless, height over diameter at breast height in the
/// same units (source 3 in the module header).
///
/// A form, not a species: there are exactly four `fn`s in `tree_species.rs`
/// and this is the growth-architecture law each of them is built on, the same
/// way `form_diameter_exponent` below is. PER-SPECIES values are a measurement
/// and belong in `data/vegetation/trees.ron` as `slenderness:` - see
/// `TreeDef::slenderness`, which overrides this whenever it is non-zero.
///
/// The four numbers, and what each one draws:
///   conifer  44 - a 22 m fir gets a 0.50 m DBH. Dominant open-grown conifers
///                 measure h/d 40-60; a plantation fir in a dense stand runs
///                 80-100 and would be a pole. Our trees stand in sparse
///                 forest, so the stout end of the dominant band is honest.
///   broadleaf 35 - an 18 m oak gets 0.51 m, an 8 m cherry 0.23 m. Open-grown
///                 temperate broadleaves run 25-45; a birch is far slenderer
///                 (55-75) and is exactly the row that wants a data value.
///   umbrella  28 - a 9 m acacia gets 0.32 m. A savanna Vachellia is a short
///                 stout bole under a wide flat crown; 22-30 is its band.
///   palm      36 - a 12 m palm gets 0.33 m, which is a coconut. Crown-of-
///                 fronds palms run 25-45; a Jubaea is genuinely stouter than
///                 that and would need its own row value.
fn form_slenderness(form: &str) -> f32 {
    match form {
        "conifer" => 44.0,
        "umbrella" => 28.0,
        "palm" => 36.0,
        _ => 35.0,
    }
}

/// Exponent `b` in `D = D_ref * (H / H_ref)^b`: how a stem thickens as an
/// individual of the same species grows taller.
///
/// 3/2 for anything with a vascular cambium (sources 1 and 2). 0 for a palm,
/// which has none - see the module header. The difference is visible in a
/// stand: jittered dicots get thicker faster than they get taller, so a big
/// oak reads as a heavier tree and not just a scaled one, while a tall palm
/// stays exactly as thick as a short palm, which is what a coconut grove
/// looks like.
fn form_diameter_exponent(form: &str) -> f32 {
    match form {
        "palm" => 0.0,
        _ => 1.5,
    }
}

/// The slenderness band a form's DRAWN BUTT may occupy: `H / (2 * r_butt)`,
/// measured off the wood that shipped by
/// `tree_radii_are_botanically_plausible`.
///
/// BUTT-referenced rather than DBH-referenced, because the butt ring is the
/// one thing the gate can measure directly on every form (there is no ring at
/// breast height on a conifer - its leader is one frustum). Each band is the
/// form's real h/d band divided by the root flare that form's builder draws:
/// `broadleaf` goes through `trunk`, which swells the butt by
/// `TRUNK_FLARE_PEAK`, so its band is the h/d band over 1.28; the other three
/// draw a plain bole straight off the ground, so theirs is the h/d band
/// unchanged.
///
///   conifer   35-100 from h/d 35-100  (no drawn root flare)
///   umbrella  22-42  from h/d 22-42   (no drawn root flare)
///   palm      25-55  from h/d 25-55   (no drawn root flare)
///   broadleaf 19.5-62.5 from h/d 25-80, over the 1.28 root flare
///
/// The bands are the real ones, so they are WIDE - that is the point. They are
/// still narrow enough that every species in the registry failed all four of
/// them before this increment (fir measured 22.7 against a floor of 35, oak
/// 13.0 against 19.5, acacia 14.7 against 22, palm 17.9 against 25).
pub(crate) fn stem_butt_slenderness_band(form: &str) -> (f32, f32) {
    match form {
        "conifer" => (35.0, 100.0),
        "umbrella" => (22.0, 42.0),
        "palm" => (25.0, 55.0),
        _ => (25.0 / TRUNK_FLARE_PEAK, 80.0 / TRUNK_FLARE_PEAK),
    }
}

/// Peak root flare `trunk` draws at the ground, as a multiple of the stem
/// radius it is handed. Named because the plausibility band has to divide by
/// it - a constant that two places must agree on is a constant, not a literal.
pub(crate) const TRUNK_FLARE_PEAK: f32 = 1.28;

/// THE RADIUS A FORM HANDS ITS BOLE BUILDER: half the diameter at breast
/// height, for a tree of this species at height `h`.
///
/// `D(h) = (H_ref / S) * (h / H_ref)^b`, with `S` the species' slenderness
/// (its own row value, else its form's) and `b` its form's thickening
/// exponent. `H_ref` is the species' nominal `height_m`, so `S` is read at the
/// size the row describes and the exponent only has to carry the jitter band
/// the spawner actually builds at.
///
/// WHAT THIS REPLACES: four flat fractions of height, one per form - 0.022 for
/// a conifer, 0.030 for a broadleaf, 0.034 for an umbrella, 0.028 for a palm.
/// They were 1.6x to 3.4x life size, they made every species of a form the
/// same tree at different scales, and being LINEAR in height they could not
/// have been right across sizes even if the constant had been.
///
/// The number returned is the DBH radius, and each form draws its own base on
/// top of it: `broadleaf` hands it to `trunk`, which swells the butt by
/// `TRUNK_FLARE_PEAK`; the other three draw a plain bole from the ground with
/// no root flare at all. MEASURED at 1.3 m on the drawn geometry, both land
/// ~5% under the DBH the law names (an 18 m oak 0.947 of it, a 22 m fir 0.950),
/// which is inside the noise of the allometry itself and not worth a per-form
/// correction factor.
pub(crate) fn stem_base_radius(def: &TreeDef, h: f32) -> f32 {
    let s = if def.slenderness > 0.0 { def.slenderness } else { form_slenderness(&def.form) };
    let h_ref = def.height_m.max(0.5);
    let d_ref = h_ref / s.max(1.0);
    0.5 * d_ref * (h.max(0.05) / h_ref).powf(form_diameter_exponent(&def.form))
}

// ── Taper: from a limb's base to a real shoot ────────────────────────────

/// Radius the OUTERMOST generation of shoots ends at, metres (v0.1090).
///
/// A cherry, maple, oak or birch last-year shoot is 5-10 mm ACROSS. Through
/// v0.1089 every limb tapered by a flat 0.68 per generation from a
/// trunk-derived base, and nothing anywhere said what a twig is: an 8 m sakura
/// ended its terminal shoots at 38 mm of RADIUS - 76 mm across, ten times life
/// size, branch-sized plumbing. That is the single loudest reason the
/// operator's v0.1088.4 close-up reads as bare tubes speared through the
/// blossom instead of as one object: no amount of card cover hides a pipe that
/// thick, and three of them fanning out of one junction visibly interpenetrate
/// for their first 15 cm.
///
/// 0.004 m of radius = 8 mm across, the middle of the real range.
pub(crate) const TWIG_TIP_R_M: f32 = 0.004;

/// A limb this thin at its tip is a SHOOT, and shoots carry foliage (v0.1096).
///
/// Through v0.1095 foliage was keyed on `depth + 1 >= max_depth` - a GENERATION
/// NUMBER - which only works while every limb in the tree is planned to the
/// same depth. Once laterals get their own `max_depth` from their own length
/// (see `generations_for`), a generation number stops meaning anything: a 4 m
/// lower lateral's third generation is a 1.4 m branch and a 1.2 m upper
/// lateral's third generation is a 30 cm shoot. Radius is the physical fact
/// that does mean something, so it is what the rule reads. This is a WIDENING
/// of the old rule, never a narrowing: the last two generations still carry
/// foliage exactly as they did, and any limb that thins to a shoot earlier now
/// carries it too, which is what fills a deep crown's interior instead of
/// leaving a bald cone inside a shell of tips.
pub(crate) const FOLIAGE_TIP_R_M: f32 = TWIG_TIP_R_M * 2.0;

/// The band a TERMINAL shoot's radius has to land in, metres, for every
/// species and every form. Read by `tree_radii_are_botanically_plausible`.
///
/// 1.2-6.0 mm of radius is 2.4-12 mm ACROSS. A real last-year shoot on a
/// temperate broadleaf is 3-8 mm across; a vigorous leader shoot or a conifer
/// branchlet reaches 10-12, and a fine cherry twiglet drops to 3. The band is
/// the honest full spread, and it still fails every pre-v0.1103 form that did
/// not route its tip through `limb_tip_radius`: the acacia measured 11.3 mm of
/// radius and the fir 55 mm.
pub(crate) const SHOOT_R_MAX_M: f32 = 0.006;
pub(crate) const SHOOT_R_MIN_M: f32 = 0.0012;

/// Length a terminal shoot should come out at, metres. See `generations_for`.
const TERMINAL_SHOOT_M: f32 = 0.55;

/// Mean length ratio between a limb and its children, measured off `limb`'s own
/// `child_len` draws (0.62-0.78 early, 0.42-0.56 for the last two generations).
const CHILD_LEN_MEAN: f32 = 0.60;

/// How many generations a limb of `len_m` needs to end in a real shoot.
///
/// Length falls by roughly `CHILD_LEN_MEAN` per generation, so the count that
/// lands on `TERMINAL_SHOOT_M` is `ln(len / shoot) / ln(1 / ratio)`. A 3 m
/// lower lateral wants 3, a 1.3 m upper one wants 2 - which is the whole point
/// of shedding laterals up a leader instead of fanning three equal primaries
/// off one point: a short high lateral must not be subdivided as if it were a
/// long low one, or the crown costs four times the wood for the same silhouette.
///
/// Clamped to 3 at the top ON PURPOSE, not for realism: each extra generation
/// multiplies the twig count by ~2.45, and the twig count is what the cluster
/// card planner spends. At 4 the card layer overruns `CARD_TRI_BUDGET` and the
/// stretch backstop thins every sleeve to one station, which is exactly the
/// bare-wood-at-the-junction defect v0.1090 removed. Measured, not guessed -
/// the `[lai]` line in `cluster_cards_reach_target_lai_and_fit_the_budget`
/// prints the card triangles every CI run.
pub(crate) fn generations_for(len_m: f32) -> u32 {
    let ratio = 1.0 / CHILD_LEN_MEAN;
    let g = (len_m.max(TERMINAL_SHOOT_M) / TERMINAL_SHOOT_M).ln() / ratio.ln();
    g.round().clamp(2.0, 3.0) as u32
}

/// Tip radius of a limb at `depth` whose base radius is `r0`.
///
/// GEOMETRIC toward `TWIG_TIP_R_M` across the generations that are left, so the
/// species' own trunk scale still decides how thick a PRIMARY limb is while
/// every tree, at every size, ends in a real twig. `max_depth` is the last
/// generation built, so `max_depth - depth` generations follow this one.
///
/// Traced on the shipped meshes at the v0.1103 radii: an 8 m sakura's lowest
/// lateral runs 10.6 -> 5.6 -> 2.4 -> 1.2 -> 0.5 cm ACROSS, which is a cherry;
/// an 18 m oak's runs 24 -> 10 -> 3.6 -> 1.5 -> 0.6 cm, which is an oak. The
/// flat 0.68 ratio this replaced produced 26 -> 18 -> 12 -> 8 cm on the cherry:
/// every generation nearly as fat as the one before it, which is a pipe organ.
///
/// v0.1103: this is now the ONLY tip law in the generator. Through v0.1102 it
/// was reached by `limb` alone, so the two forms that draw a limb as a single
/// frustum - the conifer's whorl branches and the umbrella's crown fans - took
/// hardcoded fractions of their own base instead (0.14 and 0.35) and ended at
/// 55 mm and 11.3 mm of radius. Those are the twigs cluster cards have to
/// sleeve, which is why fir and acacia could not be given a canopy at all.
pub(crate) fn limb_tip_radius(r0: f32, depth: u32, max_depth: u32) -> f32 {
    let left = max_depth.saturating_sub(depth);
    if left == 0 {
        // The terminal shoot itself: land ON the physical twig radius, and
        // never taper UP if a species is small enough to start below it.
        return TWIG_TIP_R_M.min(r0 * 0.55);
    }
    let ratio = (TWIG_TIP_R_M / r0.max(1e-4)).clamp(1e-4, 1.0);
    (r0 * ratio.powf(1.0 / (left + 1) as f32)).max(TWIG_TIP_R_M)
}

/// A limb's PLAIN TAPER radius `x` metres along its own spine from the root
/// ring: what it would be with no base flare at all.
///
/// `r0` is the SHAFT radius (what the limb settles at once the flare has
/// decayed), `r1` its tip radius, `len` its length. The flare rides on top of
/// this per VERTEX (`Flare::mul`) rather than per station, because a real
/// branch base is an ellipse - see the block comment in the kernel.
///
/// LINEAR, and deliberately still linear after the v0.1103 audit. A real limb
/// between two forks is close to a truncated cone: the paraboloid and neiloid
/// forms that classical stem-taper work (Behre, Kozak) fits are descriptions
/// of a WHOLE bole from butt to tip, and this function only ever spans one
/// internode of the architecture, over which the difference is under a
/// millimetre. The taper defect was never the curve - it was that `r1` did not
/// reach shoot scale on two of the four forms.
pub(crate) fn limb_base_radius_at(x: f32, len: f32, r0: f32, r1: f32) -> f32 {
    let t = (x / len.max(1e-4)).clamp(0.0, 1.0);
    r0 + (r1 - r0) * t
}

// ── How wood divides: forks against laterals ─────────────────────────────

/// Exponent of the pipe model (da Vinci's rule): the cross-sectional area of a
/// limb equals the summed area of the limbs it forks into, so
/// `r_parent^D = sum(r_child^D)`.
///
/// D = 2.0 is exact area conservation (Leonardo's own statement, and the pipe
/// model of Shinozaki 1964); field measurements on temperate trees put it a
/// little higher, 2.0-2.5, because a fork also has to be mechanically stiff.
/// 2.3 is the middle of the measured band: a two-way fork's children are 0.74
/// of the parent, a three-way 0.62, the acacia's five-way 0.50.
const FORK_AREA_EXP: f32 = 2.3;

/// Shaft radius of one of `siblings` limbs FORKING off a limb that ended at
/// `parent_tip_r` - i.e. a junction where the parent stops and its wood is
/// divided among the children.
pub(crate) fn fork_child_radius(parent_tip_r: f32, siblings: u32) -> f32 {
    parent_tip_r * (siblings.max(1) as f32).powf(-1.0 / FORK_AREA_EXP)
}

/// Shaft radius of a LATERAL shed off an axis that CONTINUES past it - a
/// conifer's whorl branch, sized from its own length rather than from the
/// leader it leaves (v0.1103).
///
/// THE DISTINCTION THIS MAKES, AND WHY IT IS NOT A DETAIL. A FORK divides
/// existing wood, so the pipe model sizes it: five acacia primaries off a bole
/// each get `5^(-1/2.3)` of it. A LATERAL takes nothing away from an axis that
/// keeps going; its cross-section is set by the leaf area IT supports
/// (Shinozaki's pipe model in its original form). For a branch bearing foliage
/// along a spray whose width grows with its length, that leaf area goes as
/// `L^2`, so basal AREA goes as `L^2` and basal DIAMETER goes as `L` - a
/// branch's thickness is proportional to its own length, not to the trunk
/// beside it.
///
/// The coefficient is the measured one. Crown-structure work on Douglas-fir
/// (Maguire, Kershaw & Hann, Forest Science 37, 1991) puts the largest branch
/// on a dominant conifer at 0.10-0.20 of DBH, and branch diameter falling
/// through the crown in step with branch length. 0.007 of radius per metre of
/// length is 1.4% of length in DIAMETER: a 5.8 m bottom-whorl branch on a 22 m
/// fir comes out 81 mm thick against a 0.50 m DBH (0.16 of it, mid-band), and
/// the 0.9 m branch at the apex comes out 13 mm. Both are what you measure on
/// a real fir.
///
/// What it replaces was `0.42 * the local leader radius`, which off the old
/// oversized trunk gave that same bottom branch 331 mm and the apex branch
/// 79 mm - 4x and 6x life size, on the 45 branches that ARE the silhouette of
/// a conifer. (Even on a correctly sized trunk that fraction would still have
/// drawn 197 mm: the trunk was only half of this defect.)
///
/// Capped at `cap_frac` of the parent so a lateral can never be thicker than
/// the axis it leaves, which is a precondition of the weld
/// (`flare_gain_at` has no room to swell into a parent smaller than its child)
/// and of `no_branch_pokes_out_the_back_of_its_parent`.
pub(crate) fn lateral_branch_radius(len_m: f32, parent_r: f32, cap_frac: f32) -> f32 {
    const R_PER_LEN: f32 = 0.007;
    (R_PER_LEN * len_m.max(0.05)).min(parent_r * cap_frac).max(TWIG_TIP_R_M * 1.5)
}

#[cfg(test)]
mod tests {
    // The gate for the laws above, and it lives beside them for the reason
    // every gate in this crate should: a law and its evidence drift apart the
    // moment they sit in different files. The three helpers are borrowed from
    // the KERNEL's test module rather than re-written here - one registry
    // loader, one procedural-ification, one seed - because they have to be the
    // ones every other tree gate uses or this one is measuring a different
    // tree from the rest of them.
    use super::super::tests::{as_procedural, registry, shipped_seed};
    use super::super::{build_accepted, stem_base_radius, stem_butt_slenderness_band};
    use super::{SHOOT_R_MAX_M, SHOOT_R_MIN_M, TRUNK_FLARE_PEAK};

    /// THE BOTANICAL PLAUSIBILITY GATE (v0.1103): are these radii METRES that
    /// a real tree of this height and this growth form actually measures?
    ///
    /// WHY IT DID NOT EXIST, AND WHAT ITS ABSENCE COST. Every other radius gate
    /// in the kernel measures a limb AGAINST ITSELF or against its parent - the
    /// flare gate checks weld-over-shaft, the back-poke gate checks child
    /// against parent wall, the taper gate checks monotonicity. Every one of
    /// them is SCALE-FREE, so all six stay green on a tree whose every radius
    /// is twice life size. Nothing anywhere asserted a metre.
    ///
    /// So a 22 m fir shipped with a 0.484 m trunk RADIUS - a 0.97 m bole where
    /// a real Abies that tall is 0.50-0.55 m through - and an acacia's terminal
    /// twigs ended at 11.3 mm of radius, 23 mm across, against a real last-year
    /// shoot at 3-8 mm. That is the operator's "branches look like plumbing"
    /// report, and it is why the junction collar was reshaped twice chasing a
    /// defect the collar never had.
    ///
    /// TWO MEASUREMENTS, both taken off the wood that SHIPPED:
    ///
    ///   1. THE BUTT. Take the widest drawn ring at ground level and read
    ///      `H / (2 * r)` - forestry's slenderness, butt-referenced. Each
    ///      form's real band is in `stem_butt_slenderness_band`. Cross-checked
    ///      against `stem_base_radius` in the same breath, so the gate proves
    ///      the LAW and the GEOMETRY are the same tree; without that half, a
    ///      builder that ignored the law could still pass by luck.
    ///   2. THE TERMINAL SHOOT. Every twig the card planner is handed and told
    ///      is a tip has to END at shoot scale, `SHOOT_R_MIN_M..SHOOT_R_MAX_M`,
    ///      on EVERY form. This is the measurement that gates a species'
    ///      cluster cards: a card is sized to sleeve a twig, so a twig 4.5x too
    ///      fat demands a card too big for its own foliage.
    ///
    /// Run at three heights per species - the jitter band the spawner actually
    /// builds at - so an allometry that only happens to be right at the nominal
    /// height cannot pass.
    ///
    /// PROVEN TO FAIL ON PURPOSE (2026-08-03, the pre-v0.1103 constants put
    /// back and the gate re-run): ALL 24 of the 24 (species, height) builds
    /// failed, 60 violations. Every species missed its slenderness band - fir
    /// and pine at 22.7 against a floor of 35, the four broadleaves at 13.0
    /// against 19.5, acacia at 14.7 against 22, palm at 17.9 against 25 - and
    /// the fir, acacia and palm tip asserts fired at 55.2, 11.3 and 12.2 mm of
    /// radius against a 6.0 mm ceiling. A gate that cannot be shown to fail for
    /// the defect it names has not been tested, only written.
    #[test]
    fn tree_radii_are_botanically_plausible() {
        let r = registry();
        let mut checked = 0usize;
        // COLLECTED, not asserted one at a time: the defect this gate exists
        // for was systemic (all eight species, both ends of the tree), and a
        // gate that stops at the first one turns a diagnosis into a queue.
        let mut bad: Vec<String> = Vec::new();
        for t in r.trees.iter().map(as_procedural) {
            let (lo, hi) = stem_butt_slenderness_band(&t.form);
            let j = t.height_jitter.clamp(0.0, 0.4);
            for hs in [1.0 - j, 1.0, 1.0 + j] {
                let h = t.height_m * hs;
                let (parts, twigs) = build_accepted(&t, h, shipped_seed(0));

                // 1. THE BUTT, off the drawn wood rather than off the constant.
                let butt = parts
                    .wood
                    .vertices
                    .iter()
                    .filter(|v| v.position[1] <= h * 0.012)
                    .fold(0.0f32, |a, v| a.max(v.position[0].hypot(v.position[2])));
                assert!(butt > 0.0, "{}: no drawn wood at ground level", t.id);
                let slender = h / (2.0 * butt);

                // 2. THE TERMINAL SHOOT.
                let tips: Vec<f32> = twigs.iter().filter(|w| w.tip).map(|w| w.tip_r).collect();
                assert!(tips.len() > 5, "{}: only {} terminal twigs to measure", t.id, tips.len());
                let fat = tips.iter().cloned().fold(0.0f32, f32::max);
                let thin = tips.iter().cloned().fold(f32::MAX, f32::min);
                eprintln!(
                    "[allometry] {:>7} {:>9} h {h:5.1} m: butt r {butt:.3} m ({:.2} m across), \
                     h/2r {slender:5.1} (band {lo:.0}-{hi:.0}); {} shoots end {:.1}-{:.1} mm \
                     radius ({:.0}-{:.0} mm across)",
                    t.id,
                    t.form,
                    butt * 2.0,
                    tips.len(),
                    thin * 1000.0,
                    fat * 1000.0,
                    thin * 2000.0,
                    fat * 2000.0
                );
                if !(lo..=hi).contains(&slender) {
                    bad.push(format!(
                        "  {} at {h:.1} m: butt slenderness is {slender:.1}, outside the \
                         {lo:.0}-{hi:.0} a real {} stem occupies - a {:.2} m trunk under a \
                         {h:.1} m tree is plumbing, not wood",
                        t.id,
                        t.form,
                        butt * 2.0
                    ));
                }
                if fat > SHOOT_R_MAX_M {
                    bad.push(format!(
                        "  {} at {h:.1} m: terminal shoots reach {:.1} mm of RADIUS ({:.0} mm \
                         across) - a real last-year shoot is 3-8 mm across, and a twig this fat \
                         demands a cluster card too big for its own foliage",
                        t.id,
                        fat * 1000.0,
                        fat * 2000.0
                    ));
                }
                if thin < SHOOT_R_MIN_M {
                    bad.push(format!(
                        "  {} at {h:.1} m: terminal shoots thin to {:.2} mm of radius - that is \
                         thread, not wood, and the bark tile round it is one texel wide",
                        t.id,
                        thin * 1000.0
                    ));
                }

                // 3. THE LAW IS WHAT SHIPPED. Deliberately last: the two
                // measurements above are the defect, and a gate should name the
                // defect before it names the cause. This one ties them to
                // `stem_base_radius` so a builder that quietly stopped asking
                // the allometry - and happened to land in band anyway - cannot
                // pass. The drawn butt is the law's radius for a form that
                // draws a plain bole, and `TRUNK_FLARE_PEAK` of it for one that
                // draws a root flare (`trunk`); nothing else is legal.
                let law = stem_base_radius(&t, h);
                if !(law * 0.99..=law * TRUNK_FLARE_PEAK * 1.01).contains(&butt) {
                    bad.push(format!(
                        "  {} at {h:.1} m: the drawn butt is {butt:.3} m where the allometry says \
                         {law:.3} m (at most {:.3} m once a root flare is added) - a crown \
                         builder has stopped sizing its bole from `stem_base_radius`",
                        t.id,
                        law * TRUNK_FLARE_PEAK
                    ));
                }
                checked += 1;
            }
        }
        assert!(checked >= 24, "only {checked} (species, height) pairs reached the gate");
        assert!(
            bad.is_empty(),
            "\n\n{} of {checked} (species, height) builds are not the thickness of a real \
             tree:\n{}\n",
            bad.len(),
            bad.join("\n")
        );
    }
}
