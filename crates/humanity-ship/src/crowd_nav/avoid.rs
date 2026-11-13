//! Avoidance between people (crowd navigation rung 1; docs/design/crowd-navigation.md, section 5).

use glam::Vec2;

/// A half-plane of allowed velocities: those on the LEFT of the line through `point` along `dir`
/// (`det(dir, v - point) >= 0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub point: Vec2,
    pub dir: Vec2,
}

/// A neighbour as avoidance sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbour {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
}

/// Below this a length or a determinant counts as zero.
const EPS: f32 = 1e-5;

/// The 2D determinant (cross product) of `a` and `b`.
fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// The velocity a person at `pos` moving at `vel`, wanting `pref`, takes this step: optimal
/// reciprocal collision avoidance (van den Berg, Guy, Lin and Manocha, "Reciprocal n-Body Collision
/// Avoidance", 2011). For each neighbour, the relative velocities that meet it within `tau` seconds
/// form a truncated cone; `u` is the smallest change that leaves it, and this person takes half of
/// it, as the half-plane of velocities through `vel + u / 2` facing out along `u`. Bodies already
/// overlapping are pushed apart within the step `dt`. The `statics` (the edge of the walk) are taken
/// whole, as hard constraints. The velocity nearest `pref` inside every half-plane and under
/// `max_speed` comes from an incremental linear program; when no velocity is inside them all, the one
/// that breaks the people's half-planes least, keeping the statics.
#[allow(clippy::too_many_arguments)]
pub fn orca(pos: Vec2, vel: Vec2, pref: Vec2, radius: f32, max_speed: f32, neighbours: &[Neighbour], statics: &[Line], tau: f32, dt: f32) -> Vec2 {
    let mut lines: Vec<Line> = Vec::with_capacity(statics.len() + neighbours.len());
    lines.extend_from_slice(statics);
    let inv_tau = 1.0 / tau.max(EPS);
    for nb in neighbours {
        let rel_pos = nb.pos - pos;
        let rel_vel = vel - nb.vel;
        let dist_sq = rel_pos.length_squared();
        let r = radius + nb.radius;
        let r_sq = r * r;
        let (dir, u);
        if dist_sq > r_sq {
            // Apart: the cone of velocities that meet within tau, cut off by the circle of those
            // that meet at exactly tau.
            let w = rel_vel - rel_pos * inv_tau;
            let w_len_sq = w.length_squared();
            let dot1 = w.dot(rel_pos);
            if dot1 < 0.0 && dot1 * dot1 > r_sq * w_len_sq {
                // Nearest the cut-off circle.
                let w_len = w_len_sq.sqrt();
                let unit_w = w / w_len;
                dir = Vec2::new(unit_w.y, -unit_w.x);
                u = unit_w * (r * inv_tau - w_len);
            } else {
                // Nearest one of the cone's legs.
                let leg = (dist_sq - r_sq).sqrt();
                dir = if det(rel_pos, w) > 0.0 {
                    Vec2::new(rel_pos.x * leg - rel_pos.y * r, rel_pos.x * r + rel_pos.y * leg) / dist_sq
                } else {
                    -Vec2::new(rel_pos.x * leg + rel_pos.y * r, -rel_pos.x * r + rel_pos.y * leg) / dist_sq
                };
                u = dir * rel_vel.dot(dir) - rel_vel;
            }
        } else {
            // Overlapping: apart again within this step.
            let inv_dt = 1.0 / dt.max(EPS);
            let w = rel_vel - rel_pos * inv_dt;
            let w_len = w.length();
            let unit_w = if w_len > EPS { w / w_len } else { Vec2::X };
            dir = Vec2::new(unit_w.y, -unit_w.x);
            u = unit_w * (r * inv_dt - w_len);
        }
        lines.push(Line { point: vel + u * 0.5, dir });
    }
    let (fail, result) = program_2(&lines, max_speed, pref, false);
    if fail < lines.len() {
        program_3(&lines, statics.len(), fail, max_speed, result)
    } else {
        result
    }
}

/// The best velocity on line `no` that keeps lines `0..no` and the speed limit `radius`: nearest
/// `opt` (or furthest along it when `along`). None when there is none.
fn program_1(lines: &[Line], no: usize, radius: f32, opt: Vec2, along: bool) -> Option<Vec2> {
    let l = lines[no];
    let dot = l.point.dot(l.dir);
    let disc = dot * dot + radius * radius - l.point.length_squared();
    if disc < 0.0 {
        // The speed limit leaves nothing of the line.
        return None;
    }
    let s = disc.sqrt();
    let (mut t_left, mut t_right) = (-dot - s, -dot + s);
    for m in &lines[..no] {
        let denom = det(l.dir, m.dir);
        let numer = det(m.dir, l.point - m.point);
        if denom.abs() <= EPS {
            // Parallel: wholly on the allowed side, or wholly off it.
            if numer < 0.0 {
                return None;
            }
            continue;
        }
        let t = numer / denom;
        if denom >= 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }
        if t_left > t_right {
            return None;
        }
    }
    let t = if along {
        if opt.dot(l.dir) > 0.0 { t_right } else { t_left }
    } else {
        l.dir.dot(opt - l.point).clamp(t_left, t_right)
    };
    Some(l.point + l.dir * t)
}

/// The velocity nearest `opt` (or furthest along it when `along`, `opt` then a unit vector) that
/// keeps every line and the speed limit `radius`, and the index of the first line it could not
/// keep (`lines.len()` when it kept them all), with the best velocity found before that line.
fn program_2(lines: &[Line], radius: f32, opt: Vec2, along: bool) -> (usize, Vec2) {
    let mut result = if along {
        opt * radius
    } else if opt.length_squared() > radius * radius {
        opt.normalize_or_zero() * radius
    } else {
        opt
    };
    for i in 0..lines.len() {
        if det(lines[i].dir, lines[i].point - result) > 0.0 {
            match program_1(lines, i, radius, opt, along) {
                Some(v) => result = v,
                None => return (i, result),
            }
        }
    }
    (lines.len(), result)
}

/// When no velocity keeps every line (from line `begin` on): the velocity that breaks the people's
/// lines (from `statics` on) by the least distance, every static line kept.
fn program_3(lines: &[Line], statics: usize, begin: usize, radius: f32, mut result: Vec2) -> Vec2 {
    let mut distance = 0.0;
    for i in begin..lines.len() {
        if det(lines[i].dir, lines[i].point - result) <= distance {
            continue;
        }
        // The lines that keep within `distance` of line i, as half-planes of their own.
        let mut proj: Vec<Line> = lines[..statics].to_vec();
        for j in statics..i {
            let d = det(lines[i].dir, lines[j].dir);
            let point = if d.abs() <= EPS {
                if lines[i].dir.dot(lines[j].dir) > 0.0 {
                    // Same way: line j adds nothing.
                    continue;
                }
                (lines[i].point + lines[j].point) * 0.5
            } else {
                lines[i].point + lines[i].dir * (det(lines[j].dir, lines[i].point - lines[j].point) / d)
            };
            proj.push(Line { point, dir: (lines[j].dir - lines[i].dir).normalize_or_zero() });
        }
        let before = result;
        let opt = Vec2::new(-lines[i].dir.y, lines[i].dir.x);
        let (fail, v) = program_2(&proj, radius, opt, true);
        // A failure here is only float error: keep what there was.
        result = if fail < proj.len() { before } else { v };
        distance = det(lines[i].dir, lines[i].point - result);
    }
    result
}

/// The edge of the walk as a half-plane: an edge `dist` metres away along unit `n` may be closed on
/// no faster than reaches it in `tau_obs` seconds.
pub fn edge_line(n: Vec2, dist: f32, tau_obs: f32) -> Line {
    let s = dist.max(0.0) / tau_obs.max(1e-3);
    Line { point: n * s, dir: Vec2::new(-n.y, n.x) }
}
