//! THE GARMENTS' TEXTURES (character animation's "the look", 2026-11-09; docs/design/
//! character-animation.md section 16): each worn garment's MakeHuman textures read once, off the
//! frame, and packed into one texture the character materials sample (`renderer::materials`
//! `add_character_material`, the type 25 and 26 branches of 90-fragment-main.wgsl).
//!
//! THE PACKING. A material binds ONE albedo texture at group 3 (the layout every material shares,
//! sixteen bindings, the v0.1029 rule), so a garment's colour and its normal map share one texture,
//! side by side: the left half is the diffuse texture (times its ambient occlusion map where it has
//! one, and with its alpha: the hair's, brows' and lashes' cards), the right half the normal map.
//! The texture is sRGB, so the normal map is stored sRGB-encoded and comes back as written when
//! sampled. Each half is mipmapped on its own (no bleeding across the middle): the colour half with
//! its alpha kept at the same coverage as it shrinks for cards (`billboard_bake::build_mip_chain`,
//! Castano's method, so lashes and brows do not vanish at a few metres), straight for cloth.
//!
//! THE SIZE. The sources are 512 to 4096 texels (kept whole on disk: CLAUDE.md, space is not
//! scarce). A person three metres away covers a few hundred pixels, so each half is drawn at most
//! `characters.ron`'s `look_texture_px` (1024 by default; section 8.3 has the memory), resampled once
//! here. A texture that fails to read leaves the garment in its mean colour, and the log says why.
//!
//! Nothing is kept across runs: the pack is made in memory from the source files each run.

use std::path::Path;

/// A garment's packed texture: `levels[0]` is `2 * size` by `size` texels of sRGB RGBA8, each later
/// level half as wide and high.
#[derive(Debug, Clone)]
pub(crate) struct Packed {
    pub size: u32,
    pub levels: Vec<Vec<u8>>,
    /// The colour half's mean luminance (linear), so hair tinted by a person's hair colour keeps its
    /// strands' light and dark while its average is the colour chosen.
    pub mean_luma: f32,
    /// Whether a normal map was packed (the right half is flat otherwise).
    pub has_normal: bool,
}

fn read_rgba(path: &Path, size: u32) -> Result<image::RgbaImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
    Ok(if img.width() == size && img.height() == size { img } else { image::imageops::resize(&img, size, size, image::imageops::FilterType::Triangle) })
}

fn srgb_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

/// Read and pack a garment's textures (the module notes). `cards`: it is drawn as alpha cards
/// (hair, brows, lashes). `max_px` caps each half's size.
pub(crate) fn pack(diffuse: &Path, normal: Option<&Path>, ao: Option<&Path>, cards: bool, source_px: u32, max_px: u32) -> Result<Packed, String> {
    let size = source_px.min(max_px.max(16)).next_power_of_two().min(max_px.max(16));
    let mut colour = read_rgba(diffuse, size)?;
    if let Some(ao) = ao {
        // The ambient occlusion map multiplies the colour (in linear light); a map that does not
        // read is only a little less depth, so the garment is drawn without it.
        match read_rgba(ao, size) {
            Ok(ao) => {
                for (c, a) in colour.pixels_mut().zip(ao.pixels()) {
                    let occ = a.0[0] as f32 / 255.0;
                    for k in 0..3 {
                        c.0[k] = linear_to_srgb(srgb_to_linear(c.0[k]) * occ);
                    }
                }
            }
            Err(e) => log::warn!("Characters: a garment's ambient occlusion map did not read ({e}); drawn without it"),
        }
    }
    if !cards {
        for p in colour.pixels_mut() {
            p.0[3] = 255;
        }
    }
    let (has_normal, normals) = match normal.map(|n| read_rgba(n, size)) {
        Some(Ok(mut n)) => {
            // Stored sRGB-encoded so the sRGB texture hands back the map's own values.
            for p in n.pixels_mut() {
                for k in 0..3 {
                    p.0[k] = linear_to_srgb(p.0[k] as f32 / 255.0);
                }
                p.0[3] = 255;
            }
            (true, n)
        }
        Some(Err(e)) => {
            log::warn!("Characters: a garment's normal map did not read ({e}); drawn flat");
            (false, flat(size))
        }
        None => (false, flat(size)),
    };
    let mut lum = 0.0f64;
    let mut n = 0u64;
    for p in colour.pixels().step_by(7) {
        if p.0[3] >= 128 {
            lum += (0.2126 * srgb_to_linear(p.0[0]) + 0.7152 * srgb_to_linear(p.0[1]) + 0.0722 * srgb_to_linear(p.0[2])) as f64;
            n += 1;
        }
    }
    let mean_luma = if n > 0 { (lum / n as f64) as f32 } else { 0.5 };
    // Each half's mip chain, then side by side level by level.
    let left = if cards {
        crate::renderer::billboard_bake::build_mip_chain(colour.as_raw(), size, CARD_CUTOFF, true)
    } else {
        crate::renderer::billboard_bake::build_opaque_mip_chain(colour.as_raw(), size)
    };
    let right = crate::renderer::billboard_bake::build_opaque_mip_chain(normals.as_raw(), size);
    let count = left.len().min(right.len());
    let levels = (0..count)
        .map(|l| {
            let s = (size >> l).max(1) as usize;
            let mut out = vec![0u8; s * s * 2 * 4];
            for y in 0..s {
                out[y * s * 8..y * s * 8 + s * 4].copy_from_slice(&left[l][y * s * 4..(y + 1) * s * 4]);
                out[y * s * 8 + s * 4..(y + 1) * s * 8].copy_from_slice(&right[l][y * s * 4..(y + 1) * s * 4]);
            }
            out
        })
        .collect();
    Ok(Packed { size, levels, mean_luma, has_normal })
}

/// The alpha a card's texel counts as covered at, for the coverage-keeping mips: the opaque pass's
/// cut (`renderer::materials::HAIR_CUTOFF`'s neighbourhood; the mips keep the share of texels over it).
pub(crate) const CARD_CUTOFF: f32 = 0.5;

fn flat(size: u32) -> image::RgbaImage {
    // (0.5, 0.5, 1.0) in the map's own encoding, sRGB-encoded for the sRGB texture.
    let half = linear_to_srgb(0.5);
    image::RgbaImage::from_pixel(size, size, image::Rgba([half, half, 255, 255]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A garment's pack: twice as wide as high, its levels halving, the left half the colour and
    /// the right half the normal map handed back as written (sRGB-encoded); a card's alpha kept.
    /// Seen red 2026-11-09 with the normal half stored as read (not encoded): first it passed (the
    /// test only asked for z mostly up), so it now compares the half with the map texel by texel,
    /// which the same mistake fails: "the normal half read back 0.28713197 away from the map".
    #[test]
    fn a_garment_pack_is_colour_beside_normal() {
        let root = Path::new(env!("HUMANITY_REPO_ROOT")).join("assets/models/characters/makehuman/garments");
        let p = pack(&root.join("shoes01/shoes01_diffuse.png"), Some(&root.join("shoes01/shoes01_normal.png")), None, false, 1024, 256).expect("shoes01 packs");
        assert_eq!(p.size, 256);
        assert_eq!(p.levels[0].len(), 512 * 256 * 4);
        assert_eq!(p.levels[1].len(), 256 * 128 * 4);
        assert!(p.has_normal);
        // The normal half, sampled through sRGB, hands back the map's own values (to a hundredth):
        // the map read and resized the same way, compared texel by texel along a row.
        let map = read_rgba(&root.join("shoes01/shoes01_normal.png"), 256).expect("the map reads");
        let mut worst = 0.0_f32;
        for x in (0..256).step_by(7) {
            let px = (100 * 512 + 256 + x) * 4;
            for k in 0..3 {
                let back = srgb_to_linear(p.levels[0][px + k]);
                worst = worst.max((back - map.get_pixel(x as u32, 100).0[k] as f32 / 255.0).abs());
            }
        }
        assert!(worst < 0.01, "the normal half read back {worst} away from the map");
        let flat_back = srgb_to_linear(flat(4).get_pixel(0, 0).0[0]);
        assert!((flat_back - 0.5).abs() < 0.01, "a flat normal reads back 0.5, read {flat_back}");
        let hair = pack(&root.join("eyelashes01/eyelashes01.png"), None, None, true, 512, 128).expect("lashes pack");
        assert!(hair.levels[0].chunks(4).any(|t| t[3] < 32) && hair.levels[0].chunks(4).any(|t| t[3] > 200), "a card keeps its alpha");
    }
}
