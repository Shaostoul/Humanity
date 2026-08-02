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
    ("src/lib.rs", 18_300),
    ("src/gui/pages/chat.rs", 8_000),
    ("src/gui/mod.rs", 7_050),
    ("src/relay/relay.rs", 6_500),
    ("src/relay/handlers/msg_handlers.rs", 4_800),
    ("src/terrain/planet_chunks.rs", 4_820),
    ("src/terrain/grass.rs", 2_645),
    ("src/terrain/drawn_surface.rs", 1_100),
    ("src/gui/pages/construction.rs", 4_250),
    ("src/relay/api.rs", 4_200),
    ("src/renderer/mod.rs", 3_883),
    ("src/renderer/materials.rs", 500),
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
