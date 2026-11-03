//! Structural solver de-risk spike (v0.471).
//!
//! The construction-architecture design doc names this the #1 thing to prototype FIRST: a
//! structural-integrity pass is notoriously fragile, so we de-risk it as a pure, fully unit-tested
//! module BEFORE wiring it to geometry. It mirrors the `routing.rs` purity contract: glam + std
//! only, feature-agnostic (compiles under `relay`), and testable without linking the bin.
//!
//! This is NOT a finite-element solver. It is a believable, tunable node-beam model that
//! demonstrates the four things the real pass must do: (1) load propagates from where it is applied
//! down to anchored foundation nodes, (2) a member whose load exceeds its capacity BREAKS,
//! (3) breaks CASCADE (the load a broken member carried reroutes and can overload its neighbours),
//! and (4) a part of the structure that loses every path to ground is detected as a disconnected
//! ISLAND (it collapses). Load finds the path to ground via a hop-distance field, so removing a
//! vertical stud reroutes its load horizontally through a header to the next column down -- exactly
//! the cascade a real frame exhibits.

use glam::Vec3;

/// A connection point in the framing graph (a stud-to-plate joint, a beam end). `anchored` nodes
/// are the foundation: they carry any load to ground for free.
#[derive(Debug, Clone, Copy)]
pub struct FramingNode {
    pub pos: Vec3,
    pub anchored: bool,
    /// Dead + live load applied AT this node, in newtons (self-weight of what it carries: the floor
    /// above, the roof, occupants).
    pub load_n: f32,
}

impl FramingNode {
    pub fn anchor(pos: Vec3) -> Self {
        Self { pos, anchored: true, load_n: 0.0 }
    }
    pub fn loaded(pos: Vec3, load_n: f32) -> Self {
        Self { pos, anchored: false, load_n }
    }
}

/// A structural member (stud / joist / beam / header) spanning two nodes, with a load capacity
/// derived from its material + cross-section.
#[derive(Debug, Clone, Copy)]
pub struct FramingMember {
    pub a: usize,
    pub b: usize,
    /// Axial load capacity in newtons. Pre-compute from materials.ron via [`member_capacity`].
    pub capacity_n: f32,
}

/// Capacity (N) of a member from its material yield strength (MPa = 1e6 Pa) and cross-section (m^2).
/// e.g. a 38x89 mm stud (0.0034 m^2) of wood (25 MPa yield) = ~85 kN.
pub fn member_capacity(yield_strength_mpa: f32, cross_section_m2: f32) -> f32 {
    yield_strength_mpa * 1.0e6 * cross_section_m2
}

/// Per-member state after a solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberState {
    /// Utilization <= the deform threshold: nominal.
    Ok,
    /// Deform threshold < utilization <= 1.0: visibly straining (sag), still holding.
    Strained,
    /// Utilization > 1.0: failed.
    Broken,
}

/// The legacy three-state verdict the rest of the engine speaks (see `structural::StructuralResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralVerdict {
    Stable,
    Unstable,
    Collapsed,
}

/// Result of solving a framing graph.
#[derive(Debug, Clone)]
pub struct SolveResult {
    /// Per-member state (indexed like the input `members`).
    pub member_state: Vec<MemberState>,
    /// Resolved axial load carried by each member, in newtons.
    pub member_load: Vec<f32>,
    /// Per-node: does it still have a path to an anchor through unbroken members?
    pub supported: Vec<bool>,
    /// Nodes that lost every path to ground (a disconnected island => collapse).
    pub island_nodes: Vec<usize>,
    /// How many members broke (a measure of cascade depth).
    pub broken_count: usize,
    pub verdict: StructuralVerdict,
}

/// Utilization above which a member is "Strained" (visible sag) but still holding.
const DEFORM_THRESHOLD: f32 = 0.8;

/// Solve a framing graph: route load to ground, break overloaded members, cascade, and detect
/// disconnected islands. Deterministic (stable iteration order, no randomness).
pub fn solve(nodes: &[FramingNode], members: &[FramingMember]) -> SolveResult {
    let n = nodes.len();
    let m = members.len();
    let mut broken = vec![false; m];

    // Cascade: re-route + re-break until no NEW member fails this pass (capped at one break per
    // iteration in the worst case, so `m` iterations is the hard ceiling).
    let mut member_load = vec![0.0f32; m];
    for _ in 0..=m {
        member_load = route_load(nodes, members, &broken);
        let mut new_break = false;
        for (i, mem) in members.iter().enumerate() {
            if !broken[i] && member_load[i] > mem.capacity_n {
                broken[i] = true;
                new_break = true;
            }
        }
        if !new_break {
            break;
        }
    }

    // Support = reachable from any anchor through unbroken members (flood fill / BFS).
    let supported = flood_fill_supported(nodes, members, &broken);
    let island_nodes: Vec<usize> = (0..n)
        .filter(|&i| !supported[i] && !nodes[i].anchored && (nodes[i].load_n > 0.0 || has_member(members, i)))
        .collect();

    let member_state: Vec<MemberState> = members
        .iter()
        .enumerate()
        .map(|(i, mem)| {
            if broken[i] {
                MemberState::Broken
            } else if mem.capacity_n > 0.0 && member_load[i] / mem.capacity_n > DEFORM_THRESHOLD {
                MemberState::Strained
            } else {
                MemberState::Ok
            }
        })
        .collect();

    let broken_count = broken.iter().filter(|&&b| b).count();
    let verdict = if !island_nodes.is_empty() {
        StructuralVerdict::Collapsed
    } else if broken_count > 0 || member_state.iter().any(|s| *s == MemberState::Strained) {
        StructuralVerdict::Unstable
    } else {
        StructuralVerdict::Stable
    };

    SolveResult { member_state, member_load, supported, island_nodes, broken_count, verdict }
}

/// One load-routing pass given the current broken set. Returns the axial load each member carries.
///
/// Model: load flows from each node toward ground along the hop-distance gradient. We BFS the
/// hop-distance to the nearest anchor through unbroken members, then relax nodes from farthest to
/// nearest, each shedding its accumulated load equally among its "downhill" members (neighbours
/// strictly closer to ground). A node with no downhill path keeps its load (it will show up as an
/// island via the flood fill); an anchor absorbs whatever reaches it.
fn route_load(nodes: &[FramingNode], members: &[FramingMember], broken: &[bool]) -> Vec<f32> {
    let n = nodes.len();
    let m = members.len();
    let dist = hop_distance_to_ground(nodes, members, broken);

    // Adjacency: for each node, the (member_index, other_node) of its unbroken members.
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (i, mem) in members.iter().enumerate() {
        if broken[i] {
            continue;
        }
        adj[mem.a].push((i, mem.b));
        adj[mem.b].push((i, mem.a));
    }

    // Process nodes farthest-from-ground first so each is fully loaded before it sheds. Ties broken
    // by index for determinism. Unreachable nodes (dist == None) are skipped (islanded).
    let mut order: Vec<usize> = (0..n).filter(|&i| dist[i].is_some()).collect();
    order.sort_by(|&x, &y| {
        dist[y].cmp(&dist[x]).then(x.cmp(&y))
    });

    let mut accumulated: Vec<f32> = nodes.iter().map(|nd| nd.load_n).collect();
    let mut member_load = vec![0.0f32; m];

    for &node in &order {
        if nodes[node].anchored {
            continue; // grounded: absorbs accumulated[node]
        }
        let d = dist[node].unwrap();
        let downhill: Vec<(usize, usize)> = adj[node]
            .iter()
            .copied()
            .filter(|&(_, other)| dist[other].map_or(false, |od| od < d))
            .collect();
        if downhill.is_empty() {
            continue; // no path further down this pass; load stays here (island candidate)
        }
        let share = accumulated[node] / downhill.len() as f32;
        for (mi, other) in downhill {
            member_load[mi] += share;
            accumulated[other] += share;
        }
    }
    member_load
}

/// BFS hop-distance from the nearest anchor through unbroken members. `None` = unreachable (island).
fn hop_distance_to_ground(nodes: &[FramingNode], members: &[FramingMember], broken: &[bool]) -> Vec<Option<u32>> {
    let n = nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, mem) in members.iter().enumerate() {
        if broken[i] {
            continue;
        }
        adj[mem.a].push(mem.b);
        adj[mem.b].push(mem.a);
    }
    let mut dist = vec![None; n];
    let mut queue = std::collections::VecDeque::new();
    for (i, nd) in nodes.iter().enumerate() {
        if nd.anchored {
            dist[i] = Some(0);
            queue.push_back(i);
        }
    }
    while let Some(u) = queue.pop_front() {
        let du = dist[u].unwrap();
        for &v in &adj[u] {
            if dist[v].is_none() {
                dist[v] = Some(du + 1);
                queue.push_back(v);
            }
        }
    }
    dist
}

/// Per-node: reachable from any anchor through unbroken members.
fn flood_fill_supported(nodes: &[FramingNode], members: &[FramingMember], broken: &[bool]) -> Vec<bool> {
    hop_distance_to_ground(nodes, members, broken)
        .into_iter()
        .map(|d| d.is_some())
        .collect()
}

fn has_member(members: &[FramingMember], node: usize) -> bool {
    members.iter().any(|mem| mem.a == node || mem.b == node)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a parametric wall + floor frame across `cols` columns:
    /// - row 0 (y=0): anchored foundation nodes.
    /// - row 1 (y=3): wall nodes, each on a vertical STUD to the anchor below, tied to neighbours
    ///   by a low-capacity HEADER (a header ties the wall but is not meant to carry full vertical
    ///   load -- so a rerouted stud load snaps it).
    /// - row 2 (y=6): floor nodes carrying the upstairs load, each on a STUD down to the wall node,
    ///   tied to neighbours by a JOIST.
    /// Returns (nodes, members, king_stud_member_index) where the king stud is the middle column's
    /// wall-to-anchor stud.
    fn wall_with_floor(cols: usize) -> (Vec<FramingNode>, Vec<FramingMember>, usize) {
        let stud_cap = member_capacity(25.0, 0.0034); // wood stud ~85 kN
        let header_cap = 800.0; // newtons: a tie, not a column
        let joist_cap = member_capacity(25.0, 0.0034);
        let floor_load = 3000.0; // N per floor node (the upstairs weight)

        let mut nodes = Vec::new();
        let anchor: Vec<usize> = (0..cols).map(|c| {
            nodes.push(FramingNode::anchor(Vec3::new(c as f32, 0.0, 0.0)));
            nodes.len() - 1
        }).collect();
        let wall: Vec<usize> = (0..cols).map(|c| {
            nodes.push(FramingNode::loaded(Vec3::new(c as f32, 3.0, 0.0), 200.0)); // wall self-weight
            nodes.len() - 1
        }).collect();
        let floor: Vec<usize> = (0..cols).map(|c| {
            nodes.push(FramingNode::loaded(Vec3::new(c as f32, 6.0, 0.0), floor_load));
            nodes.len() - 1
        }).collect();

        let mut members = Vec::new();
        let mut king = 0;
        for c in 0..cols {
            // vertical studs
            let stud_idx = members.len();
            members.push(FramingMember { a: wall[c], b: anchor[c], capacity_n: stud_cap });
            if c == cols / 2 {
                king = stud_idx;
            }
            members.push(FramingMember { a: floor[c], b: wall[c], capacity_n: stud_cap });
            // horizontal ties
            if c + 1 < cols {
                members.push(FramingMember { a: wall[c], b: wall[c + 1], capacity_n: header_cap });
                members.push(FramingMember { a: floor[c], b: floor[c + 1], capacity_n: joist_cap });
            }
        }
        (nodes, members, king)
    }

    #[test]
    fn intact_frame_is_stable() {
        let (nodes, members, _king) = wall_with_floor(40);
        assert!(members.len() > 150, "a substantial frame (~200 members): {}", members.len());
        let r = solve(&nodes, &members);
        assert_eq!(r.verdict, StructuralVerdict::Stable, "intact frame stands");
        assert!(r.island_nodes.is_empty(), "no disconnected island");
        assert_eq!(r.broken_count, 0, "nothing broke");
    }

    /// Build a beam carrying `total` newtons on 3 posts to 3 anchors, optionally dropping one post.
    /// Each post is sized to carry a third safely (~74%), but NOT half.
    fn beam_on_three_posts(total: f32, drop: Option<usize>) -> (Vec<FramingNode>, Vec<FramingMember>) {
        let post_cap = 13_500.0_f32; // total/3 (=10k) is safe; total/2 (=15k) snaps it
        let nodes = vec![
            FramingNode::anchor(Vec3::new(0.0, 0.0, 0.0)),
            FramingNode::anchor(Vec3::new(2.0, 0.0, 0.0)),
            FramingNode::anchor(Vec3::new(4.0, 0.0, 0.0)),
            FramingNode::loaded(Vec3::new(2.0, 3.0, 0.0), total), // the beam node
        ];
        let beam = 3usize;
        let mut members = Vec::new();
        for anchor in 0..3usize {
            if drop == Some(anchor) {
                continue;
            }
            members.push(FramingMember { a: beam, b: anchor, capacity_n: post_cap });
        }
        (nodes, members)
    }

    /// A redistribution cascade: removing one of three posts forces the load onto the survivors past
    /// their capacity, so BOTH break (the cascade) and the beam loses all support (the island).
    #[test]
    fn removing_a_post_cascades_and_islands_the_beam() {
        let total = 30_000.0;
        let (n, m) = beam_on_three_posts(total, None);
        let intact = solve(&n, &m);
        assert_eq!(intact.verdict, StructuralVerdict::Stable, "three posts hold the beam (~74% each)");
        assert_eq!(intact.broken_count, 0);

        let (n2, m2) = beam_on_three_posts(total, Some(1)); // pull the middle post
        let collapsed = solve(&n2, &m2);
        assert!(collapsed.broken_count >= 2,
            "the two survivors overloaded and broke (cascade): {}", collapsed.broken_count);
        assert!(!collapsed.island_nodes.is_empty(), "the beam lost all support (disconnected island)");
        assert_eq!(collapsed.verdict, StructuralVerdict::Collapsed, "verdict is collapse");
    }

    #[test]
    fn capacity_from_material_strength() {
        // 250 MPa steel, 0.01 m^2 section = 2.5 MN.
        assert!((member_capacity(250.0, 0.01) - 2.5e6).abs() < 1.0);
    }

    #[test]
    fn lone_unanchored_loaded_node_is_an_island() {
        let nodes = vec![FramingNode::loaded(Vec3::new(0.0, 3.0, 0.0), 1000.0)];
        let r = solve(&nodes, &[]);
        assert_eq!(r.island_nodes, vec![0], "an unsupported loaded node is islanded");
        assert_eq!(r.verdict, StructuralVerdict::Collapsed);
    }
}
