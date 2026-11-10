//! THE GARMENTS' TEXTURES (character animation's "the look", 2026-11-09; BC7 and the slot pool
//! 2026-11-10; docs/design/character-animation.md section 16): each worn garment's texture read off
//! the frame, ready for a slot of the garment texture pool (`renderer::texture_pool`) the character
//! materials sample (`renderer::materials` types 25 and 26, `90-fragment-main.wgsl`).
//!
//! THE FILE. The converter (`scripts/gen-makehuman-look.js`, `scripts/lib/garment-texture.js`)
//! packs each garment's texture once, at conversion time: the colour (times its ambient occlusion
//! map, with its alpha for the hair's, brows' and lashes' cards) in the left half and the normal map
//! (sRGB-encoded, so the sRGB texture hands back the map's own values) in the right, each half
//! mipmapped on its own down to one block (4 texels), BC7-encoded by our own encoder, in a DDS file
//! (`garments.ron`'s `texture`). Nothing is packed or encoded here, and nothing is kept across runs:
//! the file is read each time the garment takes a slot.
//!
//! THE SIZE. Each half is `texture_px` texels at level 0 in the shipped file (1024; 512 for brows
//! and lashes, whose sources are that small). Settings > Graphics > Garment textures picks the level
//! (`gui::garment_textures`: 512, 1K, 2K, from data): at or below the shipped size the file's top
//! levels past the cap are skipped (512 uploads from the second level, a quarter of the memory);
//! above it the garment's file comes from that size's optional pack (`gui::garment_textures::packs`,
//! installed under `<data>/characters/packs/`), read here the same way with its own size, and never
//! larger than the garment's own art (`GarmentDef::art_px`).
//!
//! THE GPU. Where it offers block compression the BC7 blocks go up as they are (a byte a texel);
//! elsewhere they are decoded here to RGBA8 (`renderer::bc7`, the format's bit-exact decoder), four
//! times the memory, and Settings says so (the hardware-notice rule).

use std::path::Path;

use crate::renderer::texture_pool::{SlotClass, MIP_MIN_PX};

/// A garment's texture read for its slot: the pool shape it fits and its levels, biggest first (BC7
/// blocks, or RGBA8 decoded from them on a GPU without block compression).
#[derive(Debug, Clone)]
pub(crate) struct Loaded {
    pub class: SlotClass,
    pub levels: Vec<Vec<u8>>,
}

/// The frame step for Settings > Graphics > Garment textures (2026-11-10): the garments' sizes noted
/// for the levels' plan before the world is entered (and again every 2 s with the reuse switch off;
/// the library notes them too when it is read), then the packs' downloads and removals
/// (`gui::garment_textures::tick`). Called once a frame by the main loop, beside the model packs'.
pub(crate) fn tick(data_dir: &Path) {
    if crate::gui::garment_textures::garments_due() {
        crate::gui::garment_textures::note_garments(crate::engine::animation::read_garment_arts(data_dir));
    }
    crate::gui::garment_textures::tick(data_dir);
}

/// The levels a half of `half` texels has, down to `MIP_MIN_PX`.
fn levels_for(half: u32) -> u32 {
    (half.max(MIP_MIN_PX) / MIP_MIN_PX).ilog2() + 1
}

/// The pool shape a garment file whose halves are `texture_px` texels takes when each half is drawn
/// at most `max_px` (`SlotClass::for_half`, the pools' plan's own rule).
pub(crate) fn class_for(texture_px: u32, max_px: u32, compressed: bool) -> SlotClass {
    SlotClass::for_half(texture_px.min(max_px.max(MIP_MIN_PX)), compressed)
}

/// Read a garment's DDS for halves of at most `max_px` texels: the file's levels from the first that
/// fits; `compressed` keeps the BC7 blocks, else they are decoded to RGBA8. Errs on a file that is
/// not the converter's shape for a garment of `texture_px` (the log says why; the garment is drawn
/// in its mean colour).
pub(crate) fn read(path: &Path, texture_px: u32, max_px: u32, compressed: bool) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = crate::renderer::bc7::read_dds(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.width != texture_px * 2 || file.height != texture_px || file.levels.len() as u32 != levels_for(texture_px) {
        return Err(format!(
            "{}: {} by {} texels in {} levels, where garments.ron's {} texels a half means {} by {} in {}",
            path.display(),
            file.width,
            file.height,
            file.levels.len(),
            texture_px,
            texture_px * 2,
            texture_px,
            levels_for(texture_px)
        ));
    }
    let class = class_for(texture_px, max_px, compressed);
    let skip = (texture_px / class.height).ilog2() as usize;
    let mut levels: Vec<Vec<u8>> = file.levels.into_iter().skip(skip).collect();
    if !compressed {
        for (l, level) in levels.iter_mut().enumerate() {
            let (w, h) = ((class.width >> l).max(1), (class.height >> l).max(1));
            *level = crate::renderer::bc7::decode_level(level, w, h).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(Loaded { class, levels })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GARMENT'S FILE READS FOR EITHER SIZE AND EITHER GPU: a shipped garment (shoes01, 1024 a
    /// half) reads at Full as its whole BC7 chain (2048 by 1024, nine levels), at Low memory from its
    /// second level (1024 by 512, eight: the same pool shape as a 512 brow at Full), and on a GPU
    /// without block compression as RGBA8 decoded from the same blocks (four bytes a texel, its
    /// colour half the BC7 half's texels exactly); a brow (512) stays 512 at Low memory.
    #[test]
    fn a_garment_file_reads_for_either_size_and_either_gpu() {
        let root = Path::new(env!("HUMANITY_REPO_ROOT")).join("assets/models/characters/makehuman/garments");
        let shoes = root.join("shoes01/shoes01.dds");
        let full = read(&shoes, 1024, 1024, true).expect("shoes01 at Full");
        assert_eq!(full.class, SlotClass { width: 2048, height: 1024, levels: 9, compressed: true });
        assert_eq!(full.levels[0].len(), 2048 * 1024, "a byte a texel");
        let low = read(&shoes, 1024, 512, true).expect("shoes01 at Low memory");
        assert_eq!(low.class, SlotClass { width: 1024, height: 512, levels: 8, compressed: true });
        assert_eq!(low.levels[0], full.levels[1], "Low memory is the file's second level");
        let brow = read(&root.join("eyebrow001/eyebrow001.dds"), 512, 512, true).expect("a brow at Low memory");
        assert_eq!(brow.class, low.class, "a brow at either size shares Low memory's pool shape");
        let raw = read(&shoes, 1024, 512, false).expect("shoes01 decoded");
        assert_eq!(raw.class, SlotClass { compressed: false, ..low.class });
        assert_eq!(raw.levels[0].len(), 1024 * 512 * 4, "four bytes a texel");
        assert_eq!(raw.levels[0], crate::renderer::bc7::decode_level(&low.levels[0], 1024, 512).expect("decodes"));
        assert!(read(&shoes, 512, 512, true).is_err(), "a file that is not garments.ron's size is refused");
    }
}
