//! The navigation mesh (crowd navigation rung 1; docs/design/crowd-navigation.md, section 3).

use crate::wall_collision::{WallSegment, EYE_HEIGHT_M};
use glam::Vec2;

/// A piece of furniture's footprint: a box turned `yaw` radians about +Y (the turn
/// `Quat::from_rotation_y(yaw)` gives it), centred at `centre` (world x, z), `half` its half width
/// (x) and half depth (z) before the turn, standing between the world heights `y0` and `y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub centre: (f32, f32),
    pub half: (f32, f32),
    pub yaw: f32,
    pub y0: f32,
    pub y1: f32,
}

impl Footprint {
    /// The footprint's own frame: point (x, z) turned back by its yaw about its centre.
    fn local(&self, x: f32, z: f32) -> (f32, f32) {
        let (dx, dz) = (x - self.centre.0, z - self.centre.1);
        let (s, c) = self.yaw.sin_cos();
        // The inverse of glam's turn about +Y: x' = x cos - z sin... turned by -yaw.
        (c * dx - s * dz, s * dx + c * dz)
    }

    /// How far world point (x, z) is from the footprint's edge, metres: 0 on it, below 0 inside.
    pub fn distance(&self, x: f32, z: f32) -> f32 {
        let (lx, lz) = self.local(x, z);
        let qx = lx.abs() - self.half.0;
        let qz = lz.abs() - self.half.1;
        let outside = (qx.max(0.0).powi(2) + qz.max(0.0).powi(2)).sqrt();
        outside + qx.max(qz).min(0.0)
    }

    /// True when world point (x, z) is inside the footprint.
    pub fn contains(&self, x: f32, z: f32) -> bool {
        self.distance(x, z) < 0.0
    }

    /// The world box round the turned footprint, (min x, min z, max x, max z).
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let (s, c) = self.yaw.sin_cos();
        let ex = c.abs() * self.half.0 + s.abs() * self.half.1;
        let ez = s.abs() * self.half.0 + c.abs() * self.half.1;
        (self.centre.0 - ex, self.centre.1 - ez, self.centre.0 + ex, self.centre.1 + ez)
    }

    /// True when it stands across the knee-to-eye band of a body whose feet are at `feet_y`, the
    /// walls' rule (`WallSegment::stops_body`): a rug under the knee is walked over, a lamp over
    /// the head walked under.
    pub fn stops_body(&self, feet_y: f32) -> bool {
        self.y0 < feet_y + EYE_HEIGHT_M && self.y1 > feet_y + crate::wall_collision::KNEE_M
    }

    /// A placed machine or piece of furniture as its footprint, the way it is drawn
    /// (`engine::home_meshes::machine_mesh`): a box its size, a cylinder or a sphere its radius
    /// (`size.0`) each way, a pyramid its base, a sphere standing a radius under its centre. The one
    /// conversion the crowd's mesh and the player's walk both read (2026-11-13, the operator: "We
    /// want the ship to feel alive"), so a table that turns a walker aside stops the player too.
    pub fn of_placed(p: &crate::machines::PlacedMachine) -> Footprint {
        let (sx, sy, sz) = p.size;
        let (half, y0, y1) = match p.shape.as_str() {
            "sphere" => ((sx, sx), p.pos.1 - sx, p.pos.1 + sx),
            "cylinder" => ((sx, sx), p.pos.1, p.pos.1 + sy),
            "pyramid" => ((sx * 0.5, sx * 0.5), p.pos.1, p.pos.1 + sy),
            _ => ((sx * 0.5, sz * 0.5), p.pos.1, p.pos.1 + sy),
        };
        Footprint { centre: (p.pos.0, p.pos.2), half, yaw: p.rotation.to_radians(), y0, y1 }
    }

    /// The footprint as the walk's blockers (`wall_collision::resolve`), between its heights so the
    /// walls' rule says which bodies it stops (`WallSegment::stops_body`, the same knee-to-eye band
    /// as `stops_body` here: a rug under the knee is walked over, a table or a bed is not): its four
    /// edges, so a body sliding along it stops at its square corners, and a core along its long
    /// axis as thick as the box is narrow, so a body that finds itself inside (the piece moved onto
    /// it, a placement) is settled out the way it is out of a wall's solid box. A body inside a piece
    /// wider than about 1.45 m (its half width over the body's radius / (sqrt 2 - 1)) can be
    /// settled into a corner pocket instead; walking never takes it there, the edges hold it out.
    pub fn walls(&self) -> [WallSegment; 5] {
        let (s, c) = self.yaw.sin_cos();
        // A point of the footprint's own frame in the world (glam's turn about +Y by `yaw`).
        let world = |lx: f32, lz: f32| (self.centre.0 + c * lx + s * lz, self.centre.1 - s * lx + c * lz);
        let (hx, hz) = self.half;
        let corners = [world(-hx, -hz), world(hx, -hz), world(hx, hz), world(-hx, hz)];
        let seg = |a: (f32, f32), b: (f32, f32), half_thickness: f32| WallSegment { a, b, half_thickness, y0: self.y0, y1: self.y1, kind: crate::wall_collision::SegmentKind::Wall };
        let core = if hx >= hz { seg(world(-(hx - hz), 0.0), world(hx - hz, 0.0), hz) } else { seg(world(0.0, -(hz - hx)), world(0.0, hz - hx), hx) };
        [seg(corners[0], corners[1], 0.0), seg(corners[1], corners[2], 0.0), seg(corners[2], corners[3], 0.0), seg(corners[3], corners[0], 0.0), core]
    }
}

/// A floor to walk on: a level rectangle (world x, z), the floor of a zone, a corridor or a slab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorRect {
    pub min: (f32, f32),
    pub max: (f32, f32),
}

impl FloorRect {
    pub fn contains(&self, x: f32, z: f32) -> bool {
        x >= self.min.0 && x <= self.max.0 && z >= self.min.1 && z <= self.max.1
    }
}

/// What a navigation mesh is built from: the region (world x, z), the storey (the height of the
/// feet), what blocks a body there and where the floor is.
#[derive(Clone, Copy, Debug)]
pub struct NavInput<'a> {
    pub min: (f32, f32),
    pub max: (f32, f32),
    /// The floor's height: walls and furniture block by the walk's rule for a body standing here.
    pub feet_y: f32,
    /// Every wall, shut door, stair side and guard that might stand in the region (the ones that
    /// do not stop a body on this storey, or lie outside it, are left out as it is built).
    pub walls: &'a [WallSegment],
    /// Every piece of furniture that might stand in the region.
    pub footprints: &'a [Footprint],
    /// Where the floor is. Empty: everywhere in the region is floor.
    pub floor: &'a [FloorRect],
    /// The body's radius, metres (`BODY_RADIUS_M`).
    pub radius: f32,
}

/// How far past the region an obstacle is still read, metres: a wall just outside it still keeps
/// the cells inside clear by the radius.
const EDGE_READ_M: f32 = 1.0;

impl NavInput<'_> {
    /// The cell size the region is built with: `CELL_M`, or larger for a region that would
    /// otherwise hold more than `MAX_CELLS` cells.
    pub fn cell_for(&self) -> f32 {
        let area = ((self.max.0 - self.min.0) * (self.max.1 - self.min.1)).max(0.0);
        let at_cell = area / (super::CELL_M * super::CELL_M);
        if at_cell <= super::MAX_CELLS as f32 {
            super::CELL_M
        } else {
            super::CELL_M * (at_cell / super::MAX_CELLS as f32).sqrt()
        }
    }

    /// The walls that stop a body on this storey and reach the region.
    pub fn walls_here(&self) -> impl Iterator<Item = &WallSegment> + '_ {
        let eye = self.feet_y + EYE_HEIGHT_M;
        let (x0, z0, x1, z1) = (self.min.0 - EDGE_READ_M, self.min.1 - EDGE_READ_M, self.max.0 + EDGE_READ_M, self.max.1 + EDGE_READ_M);
        self.walls.iter().filter(move |w| {
            let r = w.half_thickness + self.radius;
            w.stops_body(eye) && w.a.0.max(w.b.0) + r >= x0 && w.a.0.min(w.b.0) - r <= x1 && w.a.1.max(w.b.1) + r >= z0 && w.a.1.min(w.b.1) - r <= z1
        })
    }

    /// The furniture that stops a body on this storey and reaches the region.
    pub fn footprints_here(&self) -> impl Iterator<Item = &Footprint> + '_ {
        let (x0, z0, x1, z1) = (self.min.0 - EDGE_READ_M, self.min.1 - EDGE_READ_M, self.max.0 + EDGE_READ_M, self.max.1 + EDGE_READ_M);
        self.footprints.iter().filter(move |f| {
            let (a, b, c, d) = f.bounds();
            f.stops_body(self.feet_y) && c + self.radius >= x0 && a - self.radius <= x1 && d + self.radius >= z0 && b - self.radius <= z1
        })
    }

    /// A fingerprint of everything the mesh is built from (FNV-1a over the numbers' bits): two
    /// inputs with the same fingerprint build the same mesh, so a mesh is kept exactly while its
    /// fingerprint holds, and anything that moves (a table pushed 1 cm, a door locked) changes it.
    /// Walls and furniture that do not reach the region or stop no body on this storey are left
    /// out, so they move without a rebuild.
    pub fn fingerprint(&self) -> u64 {
        let mut h = Fnv::new();
        for v in [self.min.0, self.min.1, self.max.0, self.max.1, self.feet_y, self.radius, self.cell_for()] {
            h.f(v);
        }
        for w in self.walls_here() {
            for v in [w.a.0, w.a.1, w.b.0, w.b.1, w.half_thickness, w.y0, w.y1] {
                h.f(v);
            }
            h.u(w.kind as u64);
        }
        h.u(0xFFFF_0001);
        for f in self.footprints_here() {
            for v in [f.centre.0, f.centre.1, f.half.0, f.half.1, f.yaw, f.y0, f.y1] {
                h.f(v);
            }
        }
        h.u(0xFFFF_0002);
        for r in self.floor {
            for v in [r.min.0, r.min.1, r.max.0, r.max.1] {
                h.f(v);
            }
        }
        h.0
    }
}

/// FNV-1a, 64 bits: a fingerprint, not a defence (nothing here is chosen by an adversary).
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn u(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn f(&mut self, v: f32) {
        self.u(v.to_bits() as u64);
    }
}

/// A rectangle of walkable cells, `x0..x1` by `z0..z1` (cell indices, the ends exclusive), and
/// where its links start in `NavMesh::links`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    pub x0: u32,
    pub z0: u32,
    pub x1: u32,
    pub z1: u32,
    first_link: u32,
    link_count: u32,
}

/// A portal: the edge two rectangles share, from `a` to `b` (world x, z), leading to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    pub to: u32,
    pub a: Vec2,
    pub b: Vec2,
}

/// No polygon (a blocked cell).
const NONE: u32 = u32::MAX;

/// The walkable floor of one storey of a region: a grid of cells, each walkable when its centre
/// keeps the body's radius clear of every obstacle and stands on a floor, merged into rectangles
/// joined by portals.
#[derive(Clone, Debug, PartialEq)]
pub struct NavMesh {
    /// The world (x, z) of cell (0, 0)'s low corner.
    pub origin: Vec2,
    pub cell: f32,
    pub w: u32,
    pub h: u32,
    pub feet_y: f32,
    pub radius: f32,
    /// Per cell: the polygon it is in, `NONE` when blocked.
    poly_of: Vec<u32>,
    pub polys: Vec<Poly>,
    links: Vec<Link>,
    /// Per polygon: which connected piece of floor it is in.
    component: Vec<u32>,
    /// Per cell: the offset (cells) to the nearest blocked cell, for avoidance's edge of the walk.
    near_blocked: Vec<(i16, i16)>,
}

impl NavMesh {
    /// Build the mesh from `input`: the floor rasterised into cells, every cell whose centre is
    /// nearer than the body's radius to a wall's face, a footprint's edge or the floor's edge
    /// blocked (exact distances), the walkable cells merged into rectangles, the portals between
    /// touching rectangles, the connected pieces, and the field of nearest blocked cells.
    pub fn build(input: &NavInput) -> NavMesh {
        let cell = input.cell_for();
        let origin = Vec2::new(input.min.0, input.min.1);
        let w = (((input.max.0 - input.min.0) / cell).ceil().clamp(1.0, 16_384.0)) as u32;
        let h = (((input.max.1 - input.min.1) / cell).ceil().clamp(1.0, 16_384.0)) as u32;
        let n = (w * h) as usize;
        let r = input.radius.max(0.0);
        let centre = |i: u32, j: u32| origin + Vec2::new(i as f32 + 0.5, j as f32 + 0.5) * cell;
        // The cells a world box (grown by `grow`) covers, clamped to the grid.
        let span = |x0: f32, z0: f32, x1: f32, z1: f32, grow: f32| {
            let lo = |v: f32, o: f32| (((v - grow - o) / cell - 0.5).floor().max(0.0)) as u32;
            let hi = |v: f32, o: f32, m: u32| ((((v + grow - o) / cell - 0.5).ceil()).max(-1.0) as i64).min(m as i64 - 1);
            (lo(x0, origin.x), lo(z0, origin.y), hi(x1, origin.x, w), hi(z1, origin.y, h))
        };

        // 1. THE FLOOR: a cell is floor when its centre is on one of the floors (all of it when
        // none are given), and walkable floor when it is the radius inside the floor's edge too.
        let mut walk = vec![input.floor.is_empty(); n];
        for f in input.floor {
            let (i0, j0, i1, j1) = span(f.min.0, f.min.1, f.max.0, f.max.1, 0.0);
            for j in j0 as i64..=j1 {
                for i in i0 as i64..=i1 {
                    let (i, j) = (i as u32, j as u32);
                    let c = centre(i, j);
                    if f.contains(c.x, c.y) {
                        walk[(j * w + i) as usize] = true;
                    }
                }
            }
        }
        // The region's own edge is no floor, so the field below sees it.
        for i in 0..w {
            walk[i as usize] = false;
            walk[((h - 1) * w + i) as usize] = false;
        }
        for j in 0..h {
            walk[(j * w) as usize] = false;
            walk[(j * w + w - 1) as usize] = false;
        }
        if !input.floor.is_empty() {
            let blocked: Vec<bool> = walk.iter().map(|&f| !f).collect();
            let near = nearest_field(&blocked, w, h);
            for (k, o) in near.iter().enumerate() {
                if walk[k] && o.0 != i16::MAX {
                    // The nearest non-floor cell's near face, not its centre.
                    let d = ((o.0 as f32).powi(2) + (o.1 as f32).powi(2)).sqrt() * cell - 0.5 * cell;
                    if d < r {
                        walk[k] = false;
                    }
                }
            }
        }

        // 2. THE WALLS AND THE FURNITURE: exact distance from each cell's centre to each one's
        // face, over the cells its box grown by the radius covers.
        for s in input.walls_here() {
            let (a, b) = (Vec2::new(s.a.0, s.a.1), Vec2::new(s.b.0, s.b.1));
            let reach = s.half_thickness + r;
            let (i0, j0, i1, j1) = span(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y), reach);
            let ab = b - a;
            let len2 = ab.length_squared();
            for j in j0 as i64..=j1 {
                for i in i0 as i64..=i1 {
                    let (i, j) = (i as u32, j as u32);
                    let k = (j * w + i) as usize;
                    if !walk[k] {
                        continue;
                    }
                    let p = centre(i, j);
                    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    if p.distance(a + ab * t) < reach {
                        walk[k] = false;
                    }
                }
            }
        }
        for f in input.footprints_here() {
            let (x0, z0, x1, z1) = f.bounds();
            let (i0, j0, i1, j1) = span(x0, z0, x1, z1, r);
            for j in j0 as i64..=j1 {
                for i in i0 as i64..=i1 {
                    let (i, j) = (i as u32, j as u32);
                    let k = (j * w + i) as usize;
                    if walk[k] {
                        let p = centre(i, j);
                        if f.distance(p.x, p.y) < r {
                            walk[k] = false;
                        }
                    }
                }
            }
        }

        // 3. THE RECTANGLES: each unclaimed walkable cell, row by row, grows a rectangle as far
        // right as the row stays walkable and unclaimed, then down while every cell under it does.
        // Every rectangle is convex, so a straight line inside one stays on the walk.
        let mut poly_of = vec![NONE; n];
        let mut polys: Vec<Poly> = Vec::new();
        for j in 0..h {
            for i in 0..w {
                let k = (j * w + i) as usize;
                if !walk[k] || poly_of[k] != NONE {
                    continue;
                }
                let free = |i: u32, j: u32, poly_of: &[u32]| {
                    let k = (j * w + i) as usize;
                    walk[k] && poly_of[k] == NONE
                };
                let mut i1 = i + 1;
                while i1 < w && free(i1, j, &poly_of) {
                    i1 += 1;
                }
                let mut j1 = j + 1;
                while j1 < h && (i..i1).all(|ii| free(ii, j1, &poly_of)) {
                    j1 += 1;
                }
                let id = polys.len() as u32;
                for jj in j..j1 {
                    for ii in i..i1 {
                        poly_of[(jj * w + ii) as usize] = id;
                    }
                }
                polys.push(Poly { x0: i, z0: j, x1: i1, z1: j1, first_link: 0, link_count: 0 });
            }
        }

        // 4. THE PORTALS: along each rectangle's east and north edges, each run of cells beyond
        // that is one other rectangle is the portal between the two, shortened by half a cell at
        // each end so a path turning at its end keeps the radius off the corner.
        let mut raw: Vec<(u32, Link)> = Vec::new();
        let mut portal = |p: u32, q: u32, a: Vec2, b: Vec2| {
            let (a, b) = if a.distance(b) > cell { (a + (b - a).normalize() * 0.5 * cell, b + (a - b).normalize() * 0.5 * cell) } else { ((a + b) * 0.5, (a + b) * 0.5) };
            raw.push((p, Link { to: q, a, b }));
            raw.push((q, Link { to: p, a, b }));
        };
        for (id, p) in polys.iter().enumerate() {
            let id = id as u32;
            if p.x1 < w {
                let x = origin.x + p.x1 as f32 * cell;
                let mut run: Option<(u32, u32)> = None;
                for z in p.z0..=p.z1 {
                    let q = if z < p.z1 { poly_of[(z * w + p.x1) as usize] } else { NONE };
                    match run {
                        Some((rq, _)) if rq == q => {}
                        Some((rq, zs)) => {
                            if rq != NONE {
                                portal(id, rq, Vec2::new(x, origin.y + zs as f32 * cell), Vec2::new(x, origin.y + z as f32 * cell));
                            }
                            run = Some((q, z));
                        }
                        None => run = Some((q, z)),
                    }
                }
            }
            if p.z1 < h {
                let z = origin.y + p.z1 as f32 * cell;
                let mut run: Option<(u32, u32)> = None;
                for x in p.x0..=p.x1 {
                    let q = if x < p.x1 { poly_of[(p.z1 * w + x) as usize] } else { NONE };
                    match run {
                        Some((rq, _)) if rq == q => {}
                        Some((rq, xs)) => {
                            if rq != NONE {
                                portal(id, rq, Vec2::new(origin.x + xs as f32 * cell, z), Vec2::new(origin.x + x as f32 * cell, z));
                            }
                            run = Some((q, x));
                        }
                        None => run = Some((q, x)),
                    }
                }
            }
        }
        raw.sort_by_key(|(p, _)| *p);
        let mut links = Vec::with_capacity(raw.len());
        let mut at = 0usize;
        for (id, poly) in polys.iter_mut().enumerate() {
            poly.first_link = links.len() as u32;
            while at < raw.len() && raw[at].0 == id as u32 {
                links.push(raw[at].1);
                at += 1;
            }
            poly.link_count = links.len() as u32 - poly.first_link;
        }

        // 5. THE CONNECTED PIECES of floor.
        let mut component = vec![NONE; polys.len()];
        let mut next = 0u32;
        let mut stack = Vec::new();
        for s in 0..polys.len() {
            if component[s] != NONE {
                continue;
            }
            component[s] = next;
            stack.push(s as u32);
            while let Some(k) = stack.pop() {
                let p = &polys[k as usize];
                for l in &links[p.first_link as usize..(p.first_link + p.link_count) as usize] {
                    if component[l.to as usize] == NONE {
                        component[l.to as usize] = next;
                        stack.push(l.to);
                    }
                }
            }
            next += 1;
        }

        // 6. THE EDGE OF THE WALK for avoidance: each cell's nearest blocked cell.
        let blocked: Vec<bool> = poly_of.iter().map(|&k| k == NONE).collect();
        let near_blocked = nearest_field(&blocked, w, h);
        NavMesh { origin, cell, w, h, feet_y: input.feet_y, radius: r, poly_of, polys, links, component, near_blocked }
    }

    /// The cell holding world point `p`, if it is in the grid.
    pub fn cell_at(&self, p: Vec2) -> Option<(u32, u32)> {
        let q = (p - self.origin) / self.cell;
        (q.x >= 0.0 && q.y >= 0.0 && (q.x as u32) < self.w && (q.y as u32) < self.h).then(|| (q.x as u32, q.y as u32))
    }

    /// The world centre of cell (i, j).
    pub fn cell_centre(&self, i: u32, j: u32) -> Vec2 {
        self.origin + Vec2::new(i as f32 + 0.5, j as f32 + 0.5) * self.cell
    }

    fn idx(&self, i: u32, j: u32) -> usize {
        (j * self.w + i) as usize
    }

    /// The polygon holding world point `p`, None when it is off the walkable floor.
    pub fn poly_at(&self, p: Vec2) -> Option<u32> {
        let (i, j) = self.cell_at(p)?;
        let k = self.poly_of[self.idx(i, j)];
        (k != NONE).then_some(k)
    }

    /// True when a body centre may stand at world point `p`.
    pub fn walkable(&self, p: Vec2) -> bool {
        self.poly_at(p).is_some()
    }

    /// The links out of polygon `k`.
    pub fn links(&self, k: u32) -> &[Link] {
        let p = &self.polys[k as usize];
        &self.links[p.first_link as usize..(p.first_link + p.link_count) as usize]
    }

    /// Which connected piece of floor polygon `k` is in: two points are joined by a walk exactly
    /// when their polygons share a component.
    pub fn component(&self, k: u32) -> u32 {
        self.component[k as usize]
    }

    /// The centre of the walkable cell nearest world point `p`, searched out to `max_m` metres;
    /// `p` itself when it is walkable already.
    pub fn nearest_walkable(&self, p: Vec2, max_m: f32) -> Option<Vec2> {
        self.nearest_walkable_where(p, max_m, |_| true)
    }

    /// `nearest_walkable` on the connected piece of floor `component` only: a start outside a wall
    /// is put on the crowd's side of it, not on the floor beyond.
    pub fn nearest_walkable_in(&self, p: Vec2, max_m: f32, component: u32) -> Option<Vec2> {
        self.nearest_walkable_where(p, max_m, |k| self.component[k as usize] == component)
    }

    fn nearest_walkable_where(&self, p: Vec2, max_m: f32, keep: impl Fn(u32) -> bool) -> Option<Vec2> {
        if self.poly_at(p).is_some_and(&keep) {
            return Some(p);
        }
        let q = (p - self.origin) / self.cell;
        let (ci, cj) = (q.x.floor() as i64, q.y.floor() as i64);
        let reach = (max_m / self.cell).ceil() as i64;
        let mut best: Option<(f32, Vec2)> = None;
        for r in 1..=reach {
            for dj in -r..=r {
                for di in -r..=r {
                    if di.abs() != r && dj.abs() != r {
                        continue;
                    }
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= self.w as i64 || j >= self.h as i64 {
                        continue;
                    }
                    let k = self.poly_of[self.idx(i as u32, j as u32)];
                    if k == NONE || !keep(k) {
                        continue;
                    }
                    let c = self.cell_centre(i as u32, j as u32);
                    let d = c.distance_squared(p);
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, c));
                    }
                }
            }
            // A ring's best is final once the next ring cannot hold anything nearer.
            if let Some((d, c)) = best {
                if d.sqrt() <= r as f32 * self.cell {
                    return Some(c);
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// The edge of the walk nearest world point `p` (on the walkable floor): the unit direction to
    /// it and how far it is, metres, read from the field of nearest blocked cells. None when no
    /// blocked cell is in reach of the field.
    pub fn nearest_edge(&self, p: Vec2) -> Option<(Vec2, f32)> {
        let (i, j) = self.cell_at(p)?;
        let (di, dj) = self.near_blocked[self.idx(i, j)];
        if di == i16::MAX {
            return None;
        }
        let b = self.cell_centre((i as i32 + di as i32) as u32, (j as i32 + dj as i32) as u32);
        let d = b - p;
        let len = d.length();
        if len < 1e-5 {
            return Some((Vec2::X, 0.0));
        }
        // To the blocked cell's near face, not its centre.
        Some((d / len, (len - 0.5 * self.cell).max(0.0)))
    }

    /// How many cells are walkable (the Dev page and the tests).
    pub fn walkable_cells(&self) -> usize {
        self.poly_of.iter().filter(|&&k| k != NONE).count()
    }
}

/// A navigation mesh kept while what it was built from holds (the reuse rule, CLAUDE.md, refined
/// 2026-10-30: in memory across frames only when tagged by its source; the tag here is the
/// fingerprint of the inputs, which any change to them changes).
#[derive(Clone, Debug, Default)]
pub struct NavCache {
    mesh: Option<NavMesh>,
    key: u64,
    /// How many times a mesh was built (the Dev page and the tests).
    pub builds: u64,
    /// What the last build took, milliseconds.
    pub last_build_ms: f32,
}

impl NavCache {
    /// Make sure the mesh is the one `input` builds: built again when the input's fingerprint
    /// moved, or always when `reuse` is false (`humanity_core::reuse::enabled()`, the Dev page's
    /// switch). True when it was built this call.
    pub fn ensure(&mut self, input: &NavInput, reuse: bool) -> bool {
        let key = input.fingerprint();
        if reuse && self.mesh.is_some() && key == self.key {
            return false;
        }
        let t0 = std::time::Instant::now();
        self.mesh = Some(NavMesh::build(input));
        self.key = key;
        self.builds += 1;
        self.last_build_ms = t0.elapsed().as_secs_f32() * 1000.0;
        true
    }

    pub fn mesh(&self) -> Option<&NavMesh> {
        self.mesh.as_ref()
    }

    /// Forget the mesh (the crowd it served is gone).
    pub fn clear(&mut self) {
        self.mesh = None;
        self.key = 0;
    }
}

/// For every cell, the offset (cells) to the nearest `blocked` cell: a two-pass vector distance
/// transform (Danielsson, "Euclidean distance mapping", 1980, the 8-neighbour sequential form),
/// `(i16::MAX, i16::MAX)` where there is no blocked cell at all.
fn nearest_field(blocked: &[bool], w: u32, h: u32) -> Vec<(i16, i16)> {
    const FAR: (i16, i16) = (i16::MAX, i16::MAX);
    let (w, h) = (w as i32, h as i32);
    let mut f: Vec<(i16, i16)> = blocked.iter().map(|&b| if b { (0, 0) } else { FAR }).collect();
    let d2 = |o: (i16, i16)| if o.0 == i16::MAX { i64::MAX } else { (o.0 as i64).pow(2) + (o.1 as i64).pow(2) };
    // Take the neighbour at (i + di, j + dj)'s nearest, seen from (i, j), when it is nearer.
    let take = |f: &mut Vec<(i16, i16)>, i: i32, j: i32, di: i32, dj: i32| {
        let (ni, nj) = (i + di, j + dj);
        if ni < 0 || nj < 0 || ni >= w || nj >= h {
            return;
        }
        let o = f[(nj * w + ni) as usize];
        if o.0 == i16::MAX {
            return;
        }
        let cand = ((o.0 as i32 + di) as i16, (o.1 as i32 + dj) as i16);
        let k = (j * w + i) as usize;
        if d2(cand) < d2(f[k]) {
            f[k] = cand;
        }
    };
    for j in 0..h {
        for i in 0..w {
            take(&mut f, i, j, -1, 0);
            take(&mut f, i, j, 0, -1);
            take(&mut f, i, j, -1, -1);
            take(&mut f, i, j, 1, -1);
        }
        for i in (0..w).rev() {
            take(&mut f, i, j, 1, 0);
        }
    }
    for j in (0..h).rev() {
        for i in (0..w).rev() {
            take(&mut f, i, j, 1, 0);
            take(&mut f, i, j, 0, 1);
            take(&mut f, i, j, 1, 1);
            take(&mut f, i, j, -1, 1);
        }
        for i in 0..w {
            take(&mut f, i, j, -1, 0);
        }
    }
    f
}
