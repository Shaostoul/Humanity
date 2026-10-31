//! The plan tiles' GRID (Maps increment 3, docs/design/maps-zoom-to-anything.md 5.7): a tile is a
//! square of a level's floor in SHIP metres, keyed by (level ordinal, zoom, x, y); zoom 0 is 512 m
//! on a side at 1 m a pixel and each zoom halves it, to zoom 6 (8 m at 1.6 cm a pixel). The page
//! asks which tiles its view covers at the zoom that keeps a tile pixel no bigger than a screen
//! pixel (`zoom_for`, `keys_covering`, ship_view.rs) and names the ones it wants and has by their
//! key (`MapTiles`, gui/state_types/maps.rs); the engine bakes and keeps them
//! (engine/map_tiles.rs). Plain data and arithmetic, so it lives with the page that asks with it,
//! under the engine that bakes (moved from engine/map_tiles.rs in the crate split's rung 0, part B,
//! 2026-10-31).

/// A tile's side in pixels.
pub const TILE_PX: u32 = 512;
/// A zoom-0 tile's side, metres (1 m a pixel).
pub const TILE_BASE_M: f32 = 512.0;
/// The finest zoom: 8 m tiles, 1.6 cm a pixel.
pub const MAX_ZOOM: u8 = 6;

/// One tile: its level (ordinal), zoom, and place in the zoom's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub level: i32,
    pub z: u8,
    pub x: i32,
    pub y: i32,
}

impl TileKey {
    /// A tile's side at zoom `z`, metres.
    pub fn size_m(z: u8) -> f32 {
        TILE_BASE_M / (1u32 << z.min(MAX_ZOOM)) as f32
    }

    /// Its footprint, ship metres: [min x, min z, max x, max z].
    pub fn rect(&self) -> [f32; 4] {
        let s = Self::size_m(self.z);
        [self.x as f32 * s, self.y as f32 * s, (self.x + 1) as f32 * s, (self.y + 1) as f32 * s]
    }

    /// The tile one zoom coarser that holds this one.
    pub fn parent(&self) -> Option<TileKey> {
        (self.z > 0).then(|| TileKey { level: self.level, z: self.z - 1, x: self.x.div_euclid(2), y: self.y.div_euclid(2) })
    }

    /// Where this tile is in `outer`'s image (a tile holding it), as [u0, v0, u1, v1]: u across
    /// the ship's x, v across its z (the bake's image has the ship's -z at its top).
    pub fn uv_in(&self, outer: &TileKey) -> [f32; 4] {
        let (a, b) = (self.rect(), outer.rect());
        let s = TileKey::size_m(outer.z);
        [(a[0] - b[0]) / s, (a[1] - b[1]) / s, (a[2] - b[0]) / s, (a[3] - b[1]) / s]
    }
}

/// The zoom whose tile pixel is no bigger than a screen pixel of `m_per_px` metres (the finest
/// needed), within 0 and `MAX_ZOOM`.
pub fn zoom_for(m_per_px: f64) -> u8 {
    if !(m_per_px.is_finite() && m_per_px > 0.0) {
        return 0;
    }
    let base = (TILE_BASE_M / TILE_PX as f32) as f64;
    (base / m_per_px).log2().ceil().clamp(0.0, MAX_ZOOM as f64) as u8
}

/// The tiles of `level` at zoom `z` that cover `rect` ([min x, min z, max x, max z], ship metres).
pub fn keys_covering(level: i32, z: u8, rect: [f32; 4]) -> Vec<TileKey> {
    let s = TileKey::size_m(z);
    let (x0, y0) = ((rect[0] / s).floor() as i32, (rect[1] / s).floor() as i32);
    let (x1, y1) = (((rect[2] / s).ceil() as i32 - 1).max(x0), ((rect[3] / s).ceil() as i32 - 1).max(y0));
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push(TileKey { level, z, x, y });
        }
    }
    out
}

/// True when two footprints overlap (touching edges do not).
pub fn rects_touch(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

#[cfg(test)]
mod tests {
    //! The grid's tests (Maps increment 3). Each was seen red first; what broke it and what it said
    //! is in its comment.
    use super::*;

    /// A tile knows its footprint (zoom 0 is 512 m, each zoom halves it), the tile one zoom coarser
    /// that holds it, and where it sits in that tile's image.
    ///
    /// Seen red 2026-10-22 with the parent found by plain division (`x / 2`), which rounds a
    /// negative x toward zero: "tile (-1, -3) at zoom 2 has parent (0, -1), want (-1, -2)".
    #[test]
    fn a_tile_knows_its_footprint_and_its_parent() {
        assert_eq!(TileKey::size_m(0), 512.0);
        assert_eq!(TileKey::size_m(6), 8.0);
        let k = TileKey { level: 1, z: 2, x: 3, y: -2 };
        assert_eq!(k.rect(), [384.0, -256.0, 512.0, -128.0]);
        let p = TileKey { level: 1, z: 2, x: -1, y: -3 }.parent().unwrap();
        assert_eq!((p.x, p.y), (-1, -2), "tile (-1, -3) at zoom 2 has parent ({}, {}), want (-1, -2)", p.x, p.y);
        let parent = k.parent().unwrap();
        assert_eq!((parent.z, parent.x, parent.y), (1, 1, -1));
        assert_eq!(k.uv_in(&parent), [0.5, 0.0, 1.0, 0.5]);
        assert_eq!(TileKey { level: 0, z: 0, x: 0, y: 0 }.parent(), None);
    }

    /// The zoom keeps a tile's pixel no bigger than a screen pixel: 1 m a pixel is zoom 0, half a
    /// metre zoom 1, 0.3 m zoom 2 (a 0.25 m tile pixel), never past the finest or under the
    /// coarsest; and the tiles cover the view.
    ///
    /// Seen red 2026-10-22 with the zoom rounded down: "0.3 m a screen pixel takes zoom 1, whose
    /// 0.50 m pixels are bigger".
    #[test]
    fn the_zoom_keeps_a_tile_pixel_no_bigger_than_a_screen_pixel() {
        for (m, want) in [(1.0, 0u8), (0.5, 1), (0.3, 2), (5.0, 0), (0.001, MAX_ZOOM)] {
            let z = zoom_for(m);
            let px = TileKey::size_m(z) as f64 / TILE_PX as f64;
            assert_eq!(z, want, "{m} m a screen pixel takes zoom {z}, whose {px:.2} m pixels are bigger");
        }
        let keys = keys_covering(0, 3, [10.0, -5.0, 140.0, 60.0]);
        let xs: Vec<i32> = keys.iter().map(|k| k.x).collect();
        assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&0), Some(&2)));
        assert_eq!(keys.len(), 3 * 2, "64 m tiles over 10..140 by -5..60: three across, two down");
    }
}
