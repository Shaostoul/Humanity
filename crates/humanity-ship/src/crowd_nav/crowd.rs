//! People walking together (crowd navigation rung 1; docs/design/crowd-navigation.md, sections 4
//! to 6): each picks a destination, plans a path to it over the navigation mesh, walks it at their
//! own speed, and avoidance keeps them off each other and off the edge of the walk.

use super::avoid::{edge_line, orca, Line, Neighbour};
use super::mesh::NavMesh;
use super::path::find_path;
use glam::Vec2;

/// How a crowd walks (the design, section 5, gives each number's source). Data, not code
/// (docs/design/infinite-of-x.md): data/crowd/walking.ron holds them, any left out take these
/// defaults.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct CrowdParams {
    /// How far ahead people look for each other, seconds (ORCA's time horizon). Pedestrians'
    /// avoidance follows the time to a collision, with little reaction past about 3 s
    /// (Karamouzas, Skinner and Guy, Physical Review Letters 113, 238701, 2014).
    pub tau: f32,
    /// How far ahead people look at the edge of the walk, seconds.
    pub tau_obs: f32,
    /// How far away a neighbour is still watched, metres, and the most watched at once.
    pub neighbour_m: f32,
    pub max_neighbours: usize,
    /// Room kept round the body beyond its radius from another person, metres (personal space,
    /// rung 1's fixed share; the design, section 5.3, scales it with how crowded it is).
    pub personal_m: f32,
    /// How fast a person changes velocity, m/s2.
    pub accel: f32,
    /// How fast a person turns to face the way they walk, radians a second.
    pub turn_rate: f32,
    /// How near a destination counts as there, and a path's corner as passed, metres.
    pub arrive_m: f32,
    pub corner_m: f32,
    /// How often a walker plans their path again, seconds (being pushed off it by others).
    pub replan_s: f32,
    /// A walker who has gone less than `stuck_m` in `stuck_s` seconds gives the destination up.
    pub stuck_s: f32,
    pub stuck_m: f32,
    /// How long a walker stands at a destination before the next, seconds (from, to).
    pub dwell_s: (f32, f32),
    /// True: walkers with no destination pick one in their roam (the dev crowd). False: they
    /// stand until given one (`set_goal`).
    pub wander: bool,
}

impl Default for CrowdParams {
    fn default() -> Self {
        CrowdParams {
            tau: 2.5,
            tau_obs: 0.6,
            neighbour_m: 4.0,
            max_neighbours: 10,
            personal_m: 0.05,
            accel: 6.0,
            turn_rate: 6.0,
            arrive_m: 0.35,
            corner_m: 0.3,
            replan_s: 1.5,
            stuck_s: 4.0,
            stuck_m: 0.3,
            dwell_s: (1.0, 6.0),
            wander: true,
        }
    }
}

/// One walking person.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    /// Where the body's centre stands (world x, z) and how it moves, m/s.
    pub pos: Vec2,
    pub vel: Vec2,
    /// Which way it faces: the turn about +Y of the body's front (+Z), as the people drawn read it.
    pub facing: f32,
    pub radius: f32,
    /// Its own walking speed, m/s; 0 stands (and steps aside for others).
    pub speed: f32,
    /// Where it roams round, and how far (the dev crowd's destinations).
    pub home: Vec2,
    pub roam: f32,
    /// Where it is walking to, and the corners of the path there (`corners[0]` its start).
    pub goal: Option<Vec2>,
    pub corners: Vec<Vec2>,
    pub corner: usize,
    /// Seconds left standing before the next destination.
    pub dwell: f32,
    /// Seconds until the path is planned again.
    pub replan: f32,
    /// The progress check: where it was `stuck_t` seconds ago.
    pub stuck_at: Vec2,
    pub stuck_t: f32,
    /// Its own random numbers (xorshift), so a crowd walks the same way every run.
    pub rng: u64,
    /// Put on the walkable floor yet (a start inside furniture is moved to the nearest walkable cell).
    pub placed: bool,
    /// Destinations reached and given up (the Dev page, the tests).
    pub arrivals: u32,
    pub given_up: u32,
}

impl Default for Agent {
    fn default() -> Self {
        Agent {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            facing: 0.0,
            radius: super::BODY_RADIUS_M,
            speed: 1.34,
            home: Vec2::ZERO,
            roam: 5.0,
            goal: None,
            corners: Vec::new(),
            corner: 0,
            dwell: 0.0,
            replan: 0.0,
            stuck_at: Vec2::ZERO,
            stuck_t: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
            placed: false,
            arrivals: 0,
            given_up: 0,
        }
    }
}

impl Agent {
    /// A walker at `pos` roaming `roam` metres round `home` at `speed`, its random numbers seeded
    /// by `seed`.
    pub fn new(pos: Vec2, home: Vec2, roam: f32, speed: f32, seed: u64) -> Self {
        Agent { pos, home, roam, speed, stuck_at: pos, rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1, ..Default::default() }
    }

    /// A random number in 0..1.
    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Walk to `goal`: plan the path there now. False (and no goal) when no walk reaches it.
    pub fn set_goal(&mut self, mesh: &NavMesh, goal: Vec2, params: &CrowdParams) -> bool {
        match find_path(mesh, self.pos, goal) {
            Some(c) if c.len() >= 2 => {
                self.goal = Some(goal);
                self.corners = c;
                self.corner = 1;
                self.replan = params.replan_s * (0.75 + 0.5 * self.rand());
                self.stuck_t = 0.0;
                self.stuck_at = self.pos;
                true
            }
            _ => {
                self.goal = None;
                self.corners.clear();
                false
            }
        }
    }

    /// A destination in the roam: a walkable point joined to where it stands, at least a metre
    /// off. None after a few tries (a roam that is all furniture).
    fn pick_destination(&mut self, mesh: &NavMesh) -> Option<Vec2> {
        let here = mesh.poly_at(self.pos)?;
        let piece = mesh.component(here);
        for _ in 0..24 {
            let a = self.rand() * std::f32::consts::TAU;
            let r = self.roam * self.rand().sqrt();
            let p = self.home + Vec2::new(a.cos(), a.sin()) * r;
            if let Some(k) = mesh.poly_at(p) {
                if mesh.component(k) == piece && p.distance(self.pos) > 1.0 {
                    return Some(p);
                }
            }
        }
        None
    }

    /// What it wants to do this step: its preferred velocity.
    fn behave(&mut self, mesh: &NavMesh, h: f32, params: &CrowdParams) -> Vec2 {
        // Off the walkable floor (a table moved onto it, a mesh built anew): placed again.
        if self.placed && !mesh.walkable(self.pos) {
            self.placed = false;
        }
        if !self.placed {
            // Onto the walkable floor, on the side of any wall its roam's centre is on (a start
            // outside the room, or inside a table, is moved to the nearest spot it can walk from).
            let reach = self.roam + 4.0;
            let piece = mesh.nearest_walkable(self.home, reach).and_then(|h| mesh.poly_at(h)).map(|k| mesh.component(k));
            let spot = match piece {
                Some(c) => mesh.nearest_walkable_in(self.pos, reach, c),
                None => mesh.nearest_walkable(self.pos, reach),
            };
            if let Some(p) = spot {
                self.pos = p;
            }
            self.placed = true;
            self.stuck_at = self.pos;
        }
        if self.speed <= 0.0 {
            return Vec2::ZERO;
        }
        let Some(goal) = self.goal else {
            self.dwell -= h;
            if self.dwell > 0.0 || !params.wander {
                return Vec2::ZERO;
            }
            match self.pick_destination(mesh) {
                Some(g) if self.set_goal(mesh, g, params) => {}
                _ => self.dwell = 1.0,
            }
            return Vec2::ZERO;
        };
        // Pushed off the path by others: plan it again from here now and then.
        self.replan -= h;
        if self.replan <= 0.0 && !self.set_goal(mesh, goal, params) {
            self.dwell = 1.0;
            return Vec2::ZERO;
        }
        // Not getting anywhere (a jam in a doorway): give the destination up and stand a moment.
        self.stuck_t += h;
        if self.stuck_t >= params.stuck_s {
            if self.pos.distance(self.stuck_at) < params.stuck_m {
                self.goal = None;
                self.corners.clear();
                self.given_up += 1;
                self.dwell = 0.5 + 1.5 * self.rand();
                return Vec2::ZERO;
            }
            self.stuck_t = 0.0;
            self.stuck_at = self.pos;
        }
        let last = self.corners.len().saturating_sub(1);
        while self.corner < last && self.pos.distance(self.corners[self.corner]) < params.corner_m {
            self.corner += 1;
        }
        if self.pos.distance(goal) < params.arrive_m {
            self.goal = None;
            self.corners.clear();
            self.arrivals += 1;
            let (lo, hi) = params.dwell_s;
            self.dwell = lo + (hi - lo) * self.rand();
            return Vec2::ZERO;
        }
        let target = self.corners.get(self.corner).copied().unwrap_or(goal);
        let to = target - self.pos;
        let dist = to.length();
        if dist < 1e-4 {
            return Vec2::ZERO;
        }
        // Slowing into the destination over the last 0.8 m, as a person does.
        let slow = if self.corner >= last { (self.pos.distance(goal) / 0.8).min(1.0) } else { 1.0 };
        to / dist * self.speed * slow.max(0.2)
    }
}

/// Neighbours by cell: every agent's index sorted into a grid of `cell`-metre squares.
struct Grid {
    min: Vec2,
    cell: f32,
    w: usize,
    h: usize,
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(agents: &[Agent], cell: f32) -> Grid {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for a in agents {
            min = min.min(a.pos);
            max = max.max(a.pos);
        }
        let w = (((max.x - min.x) / cell) as usize + 1).min(4096);
        let h = (((max.y - min.y) / cell) as usize + 1).min(4096);
        let key = |p: Vec2| {
            let i = (((p.x - min.x) / cell) as usize).min(w - 1);
            let j = (((p.y - min.y) / cell) as usize).min(h - 1);
            j * w + i
        };
        let mut count = vec![0u32; w * h + 1];
        for a in agents {
            count[key(a.pos) + 1] += 1;
        }
        for k in 1..count.len() {
            count[k] += count[k - 1];
        }
        let start = count.clone();
        let mut fill = count;
        let mut items = vec![0u32; agents.len()];
        for (n, a) in agents.iter().enumerate() {
            let k = key(a.pos);
            items[fill[k] as usize] = n as u32;
            fill[k] += 1;
        }
        Grid { min, cell, w, h, start, items }
    }

    /// Every agent within the 3 x 3 cells round `p`.
    fn around(&self, p: Vec2, out: &mut Vec<u32>) {
        out.clear();
        let ci = ((p.x - self.min.x) / self.cell).floor() as i64;
        let cj = ((p.y - self.min.y) / self.cell).floor() as i64;
        for j in (cj - 1).max(0)..=(cj + 1).min(self.h as i64 - 1) {
            for i in (ci - 1).max(0)..=(ci + 1).min(self.w as i64 - 1) {
                let k = j as usize * self.w + i as usize;
                out.extend_from_slice(&self.items[self.start[k] as usize..self.start[k + 1] as usize]);
            }
        }
    }
}

/// How far past a walker's roam the mesh reaches, metres: room for its path to go round a table at
/// the roam's edge, and for the doors it walks through to be inside the mesh.
pub const REGION_MARGIN_M: f32 = 4.0;

/// The region a crowd's mesh covers (world x, z, low and high corners): round every walker's roam
/// (its home and how far it roams), `REGION_MARGIN_M` past it. None with no walkers. Never where
/// they stand: the region, and so the mesh, holds while they walk (a walker cannot leave it, its
/// edge being no floor), where taking in their places built the mesh again every frame (the rig's
/// run.log, 2026-11-13: 141 builds in 10 s).
pub fn region<'a>(agents: impl IntoIterator<Item = &'a Agent>) -> Option<(Vec2, Vec2)> {
    let mut out: Option<(Vec2, Vec2)> = None;
    for a in agents {
        let r = a.roam.max(1.0) + REGION_MARGIN_M;
        let (lo, hi) = (a.home - r, a.home + r);
        out = Some(match out {
            Some((l, h)) => (l.min(lo), h.max(hi)),
            None => (lo, hi),
        });
    }
    out
}

/// The longest step the crowd takes at once, seconds: a longer frame is taken in steps this long.
const MAX_STEP_S: f32 = 0.05;

/// Step every agent `dt` seconds on `mesh`: behaviour (destinations, paths), avoidance, and the
/// move, kept on the walkable cells.
pub fn step(mesh: &NavMesh, agents: &mut [Agent], dt: f32, params: &CrowdParams) {
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
    if dt <= 0.0 || agents.is_empty() {
        return;
    }
    let steps = (dt / MAX_STEP_S).ceil().max(1.0) as usize;
    let h = dt / steps as f32;
    let mut prefs = vec![Vec2::ZERO; agents.len()];
    let mut next = vec![Vec2::ZERO; agents.len()];
    let mut near: Vec<u32> = Vec::new();
    let mut neighbours: Vec<(f32, Neighbour)> = Vec::new();
    let mut flat: Vec<Neighbour> = Vec::new();
    for _ in 0..steps {
        for (a, p) in agents.iter_mut().zip(prefs.iter_mut()) {
            *p = a.behave(mesh, h, params);
        }
        let grid = Grid::new(agents, params.neighbour_m);
        for i in 0..agents.len() {
            let a = &agents[i];
            grid.around(a.pos, &mut near);
            neighbours.clear();
            for &j in &near {
                let j = j as usize;
                if j == i {
                    continue;
                }
                let b = &agents[j];
                let d2 = b.pos.distance_squared(a.pos);
                if d2 < params.neighbour_m * params.neighbour_m {
                    neighbours.push((d2, Neighbour { pos: b.pos, vel: b.vel, radius: b.radius + params.personal_m }));
                }
            }
            if neighbours.len() > params.max_neighbours {
                neighbours.select_nth_unstable_by(params.max_neighbours, |x, y| x.0.total_cmp(&y.0));
                neighbours.truncate(params.max_neighbours);
            }
            flat.clear();
            flat.extend(neighbours.iter().map(|n| n.1));
            let statics: Vec<Line> = match mesh.nearest_edge(a.pos) {
                Some((n, d)) if d < 1.5 => vec![edge_line(n, d, params.tau_obs)],
                _ => Vec::new(),
            };
            let max_speed = (a.speed * 1.2).max(0.6);
            next[i] = orca(a.pos, a.vel, prefs[i], a.radius + params.personal_m, max_speed, &flat, &statics, params.tau, h);
        }
        for (a, v) in agents.iter_mut().zip(next.iter()) {
            let dv = (*v - a.vel).clamp_length_max(params.accel * h);
            a.vel += dv;
            let to = a.pos + a.vel * h;
            a.pos = slide(mesh, a.pos, to, &mut a.vel);
            let speed = a.vel.length();
            if speed > 0.15 {
                let want = a.vel.x.atan2(a.vel.y);
                let mut d = (want - a.facing).rem_euclid(std::f32::consts::TAU);
                if d > std::f32::consts::PI {
                    d -= std::f32::consts::TAU;
                }
                a.facing += d.clamp(-params.turn_rate * h, params.turn_rate * h);
            }
        }
    }
}

/// The move from `from` toward `to` kept on the walkable cells: the whole move, else its x or its
/// z alone (sliding along the edge, the velocity across it dropped), else none.
fn slide(mesh: &NavMesh, from: Vec2, to: Vec2, vel: &mut Vec2) -> Vec2 {
    if mesh.walkable(to) || !mesh.walkable(from) {
        return to;
    }
    let x_only = Vec2::new(to.x, from.y);
    if mesh.walkable(x_only) {
        vel.y = 0.0;
        return x_only;
    }
    let z_only = Vec2::new(from.x, to.y);
    if mesh.walkable(z_only) {
        vel.x = 0.0;
        return z_only;
    }
    *vel = Vec2::ZERO;
    from
}
