//! Tests of the tight tree card outlines (`card_hull.rs`).

use super::*;

/// A `px` square alpha mask from a predicate on the texel centre's (u, v) (v up).
fn mask(px: u32, f: impl Fn(f32, f32) -> bool) -> Vec<u8> {
    let mut a = vec![0u8; (px * px) as usize];
    for y in 0..px {
        for x in 0..px {
            let (u, v) = ((x as f32 + 0.5) / px as f32, 1.0 - (y as f32 + 0.5) / px as f32);
            if f(u, v) {
                a[(y * px + x) as usize] = 255;
            }
        }
    }
    a
}

/// A conifer as the bake frames it: a narrow cone of crown over a trunk, in a square frame on its
/// height with a margin (the shape that fills a fifth of its tile).
fn conifer(u: f32, v: f32) -> bool {
    let (base, top) = (0.03, 0.97);
    if !(base..=top).contains(&v) {
        return false;
    }
    let crown_from = 0.15;
    let half_w = if v < crown_from { 0.02 } else { 0.21 * (top - v) / (top - crown_from) };
    (u - 0.5).abs() <= half_w
}

/// What a card samples at (u, v): the bilinear alpha of the mask (texel centres at (i + 0.5) / px,
/// clamped to the tile), the shader's `textureSampleLevel` on a linear sampler.
fn bilinear(a: &[u8], px: u32, u: f32, v: f32) -> f32 {
    let fx = u * px as f32 - 0.5;
    let fy = (1.0 - v) * px as f32 - 0.5;
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |x: f32, y: f32| {
        let x = (x as i32).clamp(0, px as i32 - 1) as u32;
        let y = (y as i32).clamp(0, px as i32 - 1) as u32;
        a[(y * px + x) as usize] as f32 / 255.0
    };
    let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1.0, y0) * tx;
    let bot = at(x0, y0 + 1.0) * (1.0 - tx) + at(x0 + 1.0, y0 + 1.0) * tx;
    top * (1.0 - ty) + bot * ty
}

/// THE CONTRACT: no sample the card can take with any alpha above zero lies outside its outline,
/// for every vertex count, so trimming never cuts a visible texel (the hashed-alpha edge lets any
/// alpha above zero through). Measured on a dense grid of sample points against the bilinear
/// sample, not against the texels the fit looked at. Seen red 2026-11-14 with `REACH_TEXELS` at
/// 0.25 (no bilinear reach): "conifer k 4: a sample at (0.479, 0.030) has alpha 0.00 outside the
/// outline" (a sliver of alpha under a hundredth, printed at two places then).
#[test]
fn the_outline_contains_every_sample_with_any_alpha() {
    let px = 256;
    let shapes: [(&str, Box<dyn Fn(f32, f32) -> bool>); 3] = [
        ("conifer", Box::new(conifer)),
        ("round crown", Box::new(|u: f32, v: f32| (u - 0.5).powi(2) + (v - 0.55).powi(2) <= 0.42f32.powi(2) || ((u - 0.5).abs() < 0.03 && v > 0.03 && v < 0.2))),
        ("leaning", Box::new(|u: f32, v: f32| v > 0.1 && v < 0.9 && (u - (0.2 + 0.5 * v)).abs() < 0.08)),
    ];
    for (name, f) in shapes.iter() {
        let a = mask(px, f);
        for k in FIT_KS {
            let h = fit(&a, px, k).expect("a covered tile has an outline");
            assert_eq!(h.n, k, "{name}: padded to exactly k");
            let steps = 700;
            for j in 0..=steps {
                for i in 0..=steps {
                    let (u, v) = (i as f32 / steps as f32, j as f32 / steps as f32);
                    let s = bilinear(&a, px, u, v);
                    assert!(s <= 0.0 || h.contains([u, v]), "{name} k {k}: a sample at ({u:.3}, {v:.3}) has alpha {s:.4} outside the outline");
                }
            }
        }
    }
}

/// THE POINT: the outline is much smaller than the square for the shapes a far tree card is, and
/// more vertices fit tighter. A conifer covers about a fifth of its tile; its eight-vertex outline,
/// and the default's, is under 0.4 of the square (the square drew 2.5 times the pixels that
/// outline does). Seen red 2026-11-14 with `fit` handing back the full square: "the 8-vertex
/// outline is 1.000 of the square".
#[test]
fn the_outline_is_much_smaller_than_the_square() {
    let px = 256;
    let a = mask(px, conifer);
    let cover = a.iter().filter(|&&x| x > 0).count() as f32 / (px * px) as f32;
    let areas: Vec<f32> = FIT_KS.iter().map(|&k| fit(&a, px, k).unwrap().area()).collect();
    assert!((0.15..0.25).contains(&cover), "the test conifer covers {cover:.3} of its tile, not a fifth");
    assert!(areas[2] < 0.40, "the 8-vertex outline is {:.3} of the square", areas[2]);
    let default = areas[FIT_KS.iter().position(|&k| k == DEFAULT_K).expect("the default is a fitted count")];
    assert!(default < 0.40, "the default {DEFAULT_K}-vertex outline is {default:.3} of the square");
    assert!(areas[0] >= areas[1] - 1.0e-4 && areas[1] >= areas[2] - 1.0e-4, "more vertices fit no tighter: {areas:?}");
    assert!(areas.iter().all(|&x| x >= cover), "an outline smaller than what it covers: {areas:?} against {cover}");
}

/// Every outline stays on its tile (a vertex past it would sample the neighbouring tile) and is
/// counter-clockwise (the fan's first winding faces the card's front, as the quad's did).
#[test]
fn outlines_stay_on_the_tile_and_wind_counter_clockwise() {
    let px = 256;
    // A crown wider than the frame's margin touches every edge.
    let a = mask(px, |u, v| (u - 0.5).abs() < 0.495 && v > 0.01 && v < 0.99);
    for k in FIT_KS {
        let h = fit(&a, px, k).unwrap();
        assert!(h.verts().iter().all(|p| p.iter().all(|&c| (0.0..=1.0).contains(&c))), "k {k}: {h:?} leaves the tile");
        assert!(h.area() > 0.0, "k {k}: clockwise or empty: {h:?}");
    }
    let c = fit(&mask(px, conifer), px, 8).unwrap();
    assert!(c.area() > 0.0, "the conifer's outline winds clockwise");
}

/// An empty tile has no outline; a short mask is refused, not read past.
#[test]
fn an_empty_tile_has_no_outline() {
    assert!(fit(&vec![0u8; 64 * 64], 64, 8).is_none());
    assert!(fit(&[255u8; 10], 64, 8).is_none());
}

/// The face's index pattern: the square's is exactly the quad the cull and the patch builder used
/// (`[0,1,2, 0,2,3, 0,2,1, 0,3,2]`), and every count is two windings of the fan.
#[test]
fn the_square_face_is_the_old_quad() {
    assert_eq!(face_indices(4), vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2]);
    for n in 3..=MAX_VERTS as u32 {
        let f = face_indices(n);
        assert_eq!(f.len() as u32, 6 * (n - 2));
        let (a, b) = f.split_at(f.len() / 2);
        for (x, y) in a.chunks(3).zip(b.chunks(3)) {
            assert_eq!([x[0], x[1], x[2]], [y[0], y[2], y[1]], "the second half is the first wound backwards");
        }
    }
}

/// The emitter puts every vertex where the old square card put the same (u, v): the polygon is a
/// trim of the same plane, so the texels land where they always did.
#[test]
fn a_trimmed_face_lies_in_the_square_faces_plane() {
    let fp = CardFootprint { frame_m: 12.285, h_nominal_m: 9.0, base_offset: 0.1337 };
    let (base, up, side) = (glam::Vec3::new(3.0, 1.0, -2.0), glam::Vec3::Y, glam::Vec3::X);
    let (mut sq_v, mut sq_i) = (Vec::new(), Vec::new());
    emit_sprite_card(base, up, side, 10.0, 5, fp, &CardHull::SQUARE, &mut sq_v, &mut sq_i);
    assert_eq!(sq_v.len(), 4);
    assert_eq!(sq_i, vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2]);
    let a = mask(256, conifer);
    let h = fit(&a, 256, 8).unwrap();
    let (mut v, mut i) = (Vec::new(), Vec::new());
    emit_sprite_card(base, up, side, 10.0, 5, fp, &h, &mut v, &mut i);
    assert_eq!((v.len(), i.len()), (8, 36));
    // The square's corners: c00 (u 0, v 0), c10, c11, c01.
    let p = |i: usize| glam::Vec3::from(sq_v[i].position);
    for (vert, &[u, w]) in v.iter().zip(h.verts()) {
        let want = p(0) + (p(1) - p(0)) * u + (p(3) - p(0)) * w;
        assert!(glam::Vec3::from(vert.position).distance(want) < 1.0e-4, "({u}, {w}) off the square's plane");
        assert!((vert.color[2] - w).abs() < 1.0e-6 && (-vert.color[1] - (6.0 + u * 0.5)).abs() < 1.0e-5, "tile and u encoding");
        assert!(vert.tree_card);
    }
}

/// The pin decides the table: 0 gives the square to every tile, a count gives every tile exactly
/// that many vertices (unbaked tiles the square padded to it, empty tiles a point), so one patch's
/// faces all match. Through the pure `table_for` (the global pin is left alone: patch builds in
/// other tests read it).
#[test]
fn the_pin_gives_every_tile_one_vertex_count() {
    let a = mask(64, conifer);
    let hulls = FIT_KS.map(|k| fit(&a, 64, k).unwrap());
    let mut g = [Fitted::Unbaked; ATLAS_TILES as usize];
    g[3] = Fitted::Hulls(hulls);
    g[4] = Fitted::Empty;
    assert!(table_for(0, &g).iter().all(|h| *h == CardHull::SQUARE));
    for (i, k) in FIT_KS.into_iter().enumerate() {
        let t = table_for(k, &g);
        assert!(t.iter().all(|h| h.n == k), "pin {k}");
        assert_eq!(t[3], hulls[i]);
        assert_eq!(t[4].area(), 0.0, "an empty tile draws nothing");
        assert_eq!(t[0].area(), 1.0, "an unbaked tile keeps the square");
    }
    assert_eq!([pin_for(0), pin_for(3), pin_for(5), pin_for(8), pin_for(200)], [0, 4, 6, 8, 8]);
}
