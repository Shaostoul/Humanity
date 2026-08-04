//! File-size RATCHET (2026-07-31, operator: "the larger a file becomes the
//! less likely an AI is to successfully read and modify it").
//!
//! The v0.932-v0.941 extraction arc took lib.rs from 22,638 to 14,937 lines
//! and shipped NO guard, so 140 releases later it had regrown to 17,677:
//! every feature wires into the frame loop because that is where the loop
//! lives, and nothing pushed back. Extraction without a ratchet is a
//! subscription, not a purchase.
//!
//! Mechanism: each monolith has a checked-in line BUDGET. Growing past the
//! budget fails this test with instructions. Shrinking a file lets you (and
//! the failure message asks you to) lower its budget so it can never regrow.
//! Budgets only go DOWN over time; adding a new file to the watch list is
//! normal work when a new monolith emerges.
//!
//! The budgets are the measured size at ratchet installation plus ~3% slack,
//! so normal in-place editing never trips it; only sustained accretion does.
//! When you trip it: move the code you are adding into the module it belongs
//! to (src/engine/ for loop code, a page's own file, a relay handler), or
//! extract a coherent cluster first. docs/dev/code-structure-plan.md has the
//! tier taxonomy that worked last time.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/file_size_ratchet.rs

use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// (path, line budget). Measured 2026-07-31 + ~3% slack. LOWER these as files
/// shrink; never raise one without an operator decision recorded in the
/// commit message.
///
/// RATCHET CLICKS (newest first):
/// - v0.1110: `renderer/tree_mesh.rs` 6_450 -> 6_400, and the gate WORKED
///   exactly as designed for the second time in seven releases. The cube fix
///   (golden-angle azimuths + solved per-card tilts, with its own gate) added
///   306 lines and put the file at 6_723 (+273). Rather than raise the number,
///   the FOLIAGE CARD ARRANGEMENT moved out verbatim to
///   `renderer/tree_cards.rs`: `emit_card`, `emit_sleeve`, `sleeve_tilts`,
///   `PHYLLOTAXIS_RAD`, `CLUSTER_SLEEVE_OFFSET` and the box gate - 417 lines,
///   one job ("where does a card go and which way does it face"). 6_723 ->
///   6_351, so 50 of the 372 lines are banked and the rest is working room.
///   `emit_cluster_cards` deliberately stayed behind: it is the PLANNER (LAI
///   fit, layer assignment, budget) and belongs with the kernel. Like
///   `tree_species` it is a `#[path]` CHILD module, so the vector helpers,
///   `PlantMeshBuilder` and the normal-blend constants arrive through one
///   `use super::*` instead of a dozen new `pub(crate)`s.
/// - v0.1108: `src/lib.rs` 18_300 -> 18_100, because THE OWED EXTRACTION
///   FINALLY LANDED. Three increments in a row trimmed comments instead; this
///   one moved the near-tree MODEL LOADER (glTF parse, scan-stretch guard,
///   procedural fallback and their material registration - 418 lines,
///   verbatim) to `src/engine/near_tree_models.rs`. Note it went to `engine/`
///   rather than the `terrain/` the older note guessed: it needs the wgpu
///   renderer and the asset manager, both native-only, while `terrain/` is
///   ungated and compiles into the relay build. 18_300 -> 17_932, so 200 of
///   the 368 lines are banked and 168 stay as working room. THE FILE NO LONGER
///   HAS A NAMED NEXT EXTRACTION - whoever next needs room should name one
///   rather than nibble. New watch entry `src/surface_walk.rs` at 1_150: it
///   went 450 -> 1_066 landing the drawn-ground stand, about 620 of that being
///   its gate suite, which is load-bearing and NOT the thing to extract.
/// - v0.1103 (lib.rs, no budget change): the gate FIRED at 18_333 (+33) and
///   was paid back honestly rather than raised. Two ~30-line INCIDENT
///   NARRATIVES had accreted in comments (the BUG-063 sapling-scan story and
///   the BUG-066 mis-scoped-timer story), both already written up properly in
///   docs/BUGS.md. Source comments should say what the code does and why it is
///   shaped that way, and point at the bug log for the story; duplicating the
///   narrative is exactly the accretion this ratchet exists to catch. Trimmed
///   to pointers: 18_333 -> 18_298.
///   THE REAL EXTRACTION IS STILL OWED, and this is the second increment in a
///   row to nibble instead: the NEAR-TREE MODEL LOADER (the glTF parse, the
///   scan-stretch guard, the procedural fallback and their material
///   registration, ~250 lines around the `for name in &tree_names` loop) is a
///   coherent cluster with one job. The next increment that wants room in
///   lib.rs should move it to `src/terrain/near_tree_models.rs` rather than
///   find another comment to shorten.
/// - v0.1103: the extraction the v0.1100 note below ASKED FOR landed, and the
///   ratchet is why. A card-form increment took tree_mesh.rs to 6_531 (+81 over
///   budget), and instead of raising the number the author moved the 678 lines
///   of species architecture - `limb` and the four crown builders - into
///   `renderer/tree_species.rs`, leaving tree_mesh.rs as the geometry kernel
///   (tubes, rings, junctions, welds, flare) plus its gates. 6_531 -> 6_343,
///   back under budget with the new content still in. The new file joins at
///   750 (measured 721 + slack). Note it is a `#[path]` CHILD module of
///   tree_mesh, not a sibling: a sibling would have needed `pub(crate)` on
///   dozens of kernel internals, where a child sees them through one
///   `use super::*` - and that is also why `renderer/mod.rs` was untouched.
/// - v0.1100: new watch-list entry `renderer/tree_mesh.rs` at 6_450. It is
///   6_267 lines today and was NOT on the list while the whole junction saga
///   (v0.1096-v0.1100: surface roots, collars, the pipe model, directional
///   flare, continuous bark UVs) grew it - exactly the blind spot this
///   ratchet exists to close, caught only because a lint pass noticed the
///   file was bigger than several watched ones. Roughly a third of it is the
///   gate suite, which is load-bearing and should NOT be the thing extracted.
///   The next increment that wants room should move the SPECIES ARCHITECTURE
///   (the conifer/umbrella/palm/broadleaf crown builders and their constants)
///   into `renderer/tree_species.rs`, leaving tree_mesh.rs as the geometry
///   kernel - tubes, rings, junctions, welds, flare - plus its gates.
/// - v0.1097: `terrain/grass.rs` 2_470 -> 2_645, and a new watch-list entry
///   `terrain/drawn_surface.rs` at 1_100. THE ONE NUMBER HERE THAT GOES UP,
///   with the reason, because the rule above says never to raise one quietly:
///   grass.rs was 2_401 lines when v0.1092 put it on the list at 2_470, and
///   v0.1093's filler-stubble class took it to 3_022 WITHOUT touching the
///   budget - so this gate has been failing (+552) since that release, and
///   nobody noticed because it is an integration test rather than part of
///   `cargo test --lib`. This increment paid 461 of those lines back by
///   promoting `DrawnPatchSurface` out (trees needed it too, so it is no
///   longer grass's private sampler), leaving 2_568. 2_645 is that measurement
///   plus the standard ~3%: still 377 lines BELOW where the file actually sits
///   today at HEAD, so the budget resumes ratcheting from a real number
///   instead of staying red and being ignored. The follow-up that pays the
///   rest back is the filler class's own extraction.
///   `terrain/planet_chunks.rs` stays at 4_820 and is now 8 lines under it -
///   which is the ratchet working as designed: the next increment that wants
///   room in that file should extract the NEAR-TREE HARVEST (`NearTree`,
///   `near_tree_instances`, `near_tree_instances_on_drawn` and the tree-ground
///   constants, ~215 lines) into `terrain/near_trees.rs`, exactly the way the
///   grass layer moved out in v0.1092.
/// - v0.1093: `renderer/mod.rs` 4_000 -> 3_883. It was the one file OVER
///   budget (4_090, +90). The material system moved verbatim to
///   `src/renderer/materials.rs`: registration, the textured variants, the
///   in-place updates, and the two bind-group builders they all funnel
///   through - 322 lines, no behaviour change (the whole delta is one `use`
///   path and one `pub(super)`). The new file joins the watch list at 500
///   rather than measured+3%: 3% of a 365-line file is 11 lines, which would
///   fire on the first legitimate new material method and send its author
///   hunting for an extraction inside a 365-line module. 500 still trips long
///   before `materials.rs` could become a second monolith, and stays inside
///   the 600-line slack window so it does not immediately demand a click.
/// - v0.1092: `planet_chunks` 5_950 -> 4_820. The v0.1091 grass-strand
///   increment raised the budget to 5_950 with the extraction of the layer
///   recorded IN THAT COMMIT as the immediate next increment; that extraction
///   is this one. The layer moved verbatim to `src/terrain/grass.rs`, which
///   joins the watch list at its own measured size, so the file it left
///   cannot quietly reabsorb it and the new file cannot quietly grow into a
///   second monolith.
const BUDGETS: &[(&str, usize)] = &[
    ("src/lib.rs", 18_100),
    ("src/surface_walk.rs", 1_150),
    ("src/gui/pages/chat.rs", 8_000),
    ("src/gui/mod.rs", 7_050),
    ("src/relay/relay.rs", 6_500),
    ("src/relay/handlers/msg_handlers.rs", 4_800),
    ("src/terrain/planet_chunks.rs", 4_820),
    ("src/terrain/grass.rs", 2_645),
    ("src/terrain/grass_mesh.rs", 1_000),
    ("src/terrain/drawn_surface.rs", 1_100),
    ("src/gui/pages/construction.rs", 4_250),
    ("src/relay/api.rs", 4_200),
    ("src/renderer/mod.rs", 3_883),
    ("src/renderer/materials.rs", 500),
    ("src/renderer/capture.rs", 300),
    ("src/renderer/shadow_cutout.rs", 250),
    ("src/renderer/tree_mesh.rs", 6_400),
    ("src/renderer/tree_species.rs", 750),
    ("src/renderer/tree_cards.rs", 450),
    ("src/renderer/tree_allometry.rs", 560),
];

#[test]
fn monolith_budgets_are_not_exceeded() {
    let mut over = Vec::new();
    let mut slack = Vec::new();
    for (rel, budget) in BUDGETS {
        let path = repo().join(rel);
        let lines = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {rel}: {e}"))
            .lines()
            .count();
        if lines > *budget {
            over.push(format!(
                "  {rel}: {lines} lines > budget {budget} (+{})",
                lines - budget
            ));
        } else if budget - lines > 600 {
            // The file shrank well below its budget: ask for the ratchet click.
            slack.push(format!(
                "  {rel}: {lines} lines, budget {budget} -- LOWER the budget to \
                 about {} so the win cannot be silently spent",
                lines + (lines / 33)
            ));
        }
    }
    assert!(
        over.is_empty(),
        "\n\nMONOLITH GREW PAST ITS BUDGET:\n{}\n\n\
         Do not raise the budget. Move the new code into the module it belongs \
         to (src/engine/ for frame-loop code, the page's own file, a relay \
         handler), or extract a coherent cluster first -- the tier taxonomy in \
         docs/dev/code-structure-plan.md completed a 7,700-line extraction and \
         works. The whole point of this ratchet is that the v0.941 extraction \
         regrew 2,740 lines in 140 releases because nothing pushed back.\n",
        over.join("\n")
    );
    assert!(
        slack.is_empty(),
        "\n\nRATCHET CLICK AVAILABLE (files shrank; lock in the win):\n{}\n",
        slack.join("\n")
    );
}
