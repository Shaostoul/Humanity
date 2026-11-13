//! The path over the navigation mesh (crowd navigation rung 1; docs/design/crowd-navigation.md,
//! section 4): A* over the rectangles, then the funnel through their portals.

use super::mesh::NavMesh;
use glam::Vec2;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// The 2D cross product of `a` and `b` (x, z): above 0 when `b` turns counter-clockwise from `a`.
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// An open polygon in the A* search, ordered so the heap pops the lowest estimate first.
#[derive(Clone, Copy, PartialEq)]
struct Open {
    f: f32,
    poly: u32,
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then_with(|| o.poly.cmp(&self.poly))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The shortest walk from `from` to `to` over `mesh`, as its corners, `from` first and `to` last;
/// None when either end is off the walkable floor or no walk joins them.
///
/// A* (Hart, Nilsson and Raphael, 1968) over the rectangles, each entered at the middle of the
/// portal it is entered by, the straight-line distance to `to` as the estimate; the corridor of
/// rectangles it finds is pulled tight through its portals by the funnel (`funnel`).
pub fn find_path(mesh: &NavMesh, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
    let start = mesh.poly_at(from)?;
    let goal = mesh.poly_at(to)?;
    if mesh.component(start) != mesh.component(goal) {
        return None;
    }
    if start == goal {
        return Some(vec![from, to]);
    }
    let n = mesh.polys.len();
    let mut g = vec![f32::INFINITY; n];
    let mut at = vec![Vec2::ZERO; n];
    // How each polygon was reached: the polygon before it and the link from that one.
    let mut came: Vec<(u32, u32)> = vec![(u32::MAX, 0); n];
    let mut closed = vec![false; n];
    let mut open = BinaryHeap::new();
    g[start as usize] = 0.0;
    at[start as usize] = from;
    open.push(Open { f: from.distance(to), poly: start });
    while let Some(Open { poly: k, .. }) = open.pop() {
        let ku = k as usize;
        if closed[ku] {
            continue;
        }
        closed[ku] = true;
        if k == goal {
            break;
        }
        for (li, l) in mesh.links(k).iter().enumerate() {
            let q = l.to as usize;
            if closed[q] {
                continue;
            }
            let m = (l.a + l.b) * 0.5;
            let mut cost = g[ku] + at[ku].distance(m);
            let estimate = if l.to == goal {
                cost += m.distance(to);
                0.0
            } else {
                m.distance(to)
            };
            if cost < g[q] {
                g[q] = cost;
                at[q] = m;
                came[q] = (k, li as u32);
                open.push(Open { f: cost + estimate, poly: l.to });
            }
        }
    }
    if !closed[goal as usize] {
        return None;
    }
    // The corridor, start to goal, as portals with their left and right ends as walked.
    let mut steps = Vec::new();
    let mut k = goal;
    while k != start {
        let (p, li) = came[k as usize];
        steps.push((p, li));
        k = p;
    }
    steps.reverse();
    let centre = |k: u32| {
        let p = &mesh.polys[k as usize];
        mesh.origin + Vec2::new((p.x0 + p.x1) as f32, (p.z0 + p.z1) as f32) * 0.5 * mesh.cell
    };
    let mut portals = Vec::with_capacity(steps.len() + 2);
    portals.push((from, from));
    for (p, li) in steps {
        let l = mesh.links(p)[li as usize];
        let d = centre(l.to) - centre(p);
        let mid = (l.a + l.b) * 0.5;
        // Left is counter-clockwise of the way walked.
        if cross(d, l.a - mid) > 0.0 {
            portals.push((l.a, l.b));
        } else {
            portals.push((l.b, l.a));
        }
    }
    portals.push((to, to));
    Some(funnel(&portals))
}

/// The shortest line through `portals` (each its left and right end as walked; the first and the
/// last are the start and the end, both ends the same point): the funnel algorithm (Lee and
/// Preparata, 1984), in the "simple stupid funnel" form (Mononen, 2010). The funnel from the apex
/// narrows portal by portal; when one side would cross the other, the crossed side's point is a
/// corner and the funnel starts again from it.
fn funnel(portals: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    let mut path = vec![portals[0].0];
    let (mut apex, mut left, mut right) = (portals[0].0, portals[0].0, portals[0].1);
    let (mut left_i, mut right_i) = (0usize, 0usize);
    let mut i = 1;
    while i < portals.len() {
        let (l, r) = portals[i];
        // The right side narrows when the new right end is counter-clockwise of it.
        if cross(right - apex, r - apex) >= 0.0 {
            if apex == right || cross(left - apex, r - apex) < 0.0 {
                right = r;
                right_i = i;
            } else {
                // Over the left side: the left end is a corner.
                path.push(left);
                apex = left;
                right = apex;
                right_i = left_i;
                i = left_i + 1;
                continue;
            }
        }
        // The left side narrows when the new left end is clockwise of it.
        if cross(left - apex, l - apex) <= 0.0 {
            if apex == left || cross(right - apex, l - apex) > 0.0 {
                left = l;
                left_i = i;
            } else {
                // Over the right side: the right end is a corner.
                path.push(right);
                apex = right;
                left = apex;
                left_i = right_i;
                i = right_i + 1;
                continue;
            }
        }
        i += 1;
    }
    let end = portals[portals.len() - 1].0;
    if path.last() != Some(&end) {
        path.push(end);
    }
    path
}
