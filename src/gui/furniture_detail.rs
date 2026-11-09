//! FURNITURE DETAIL (2026-11-09): Settings > Graphics > Furniture detail, and the hardware notice
//! beside it (docs/design/simready-props.md, sections 9 and 10).
//!
//! Every machine and piece of furniture in a home may draw a model (the Kenney furniture, the NVIDIA
//! SimReady props) instead of its plain box. Their textures and meshes take video memory, so the
//! levels are data (data/gui/furniture_detail.json): Full draws each model's texture as it ships,
//! Simple halves each side of it (a quarter of the texture memory), Boxes draws the plain boxes,
//! and Auto picks Full or Simple from the graphics card's own memory. These are also the dual-mode
//! rule's realistic and simplified modes (CLAUDE.md, "Every deep system ships two modes").
//!
//! The memory is MEASURED, not guessed: the home's body build (`home_meshes::build_machine_bodies`)
//! records what its models' textures and meshes take at the level it drew ([`note_model_memory`]),
//! and the Settings page shows each level's figure from that: Full's as measured, Simple's the
//! meshes and a quarter of the textures, Boxes none. With the card's memory beside it, a level that
//! would take more than the file's share of the card is said in words, what it needs, what the card
//! has, and what is on instead (operator, 2026-11-08: "We should make sure that we clearly notice and
//! inform users when they have a GPU or whatever that can't run something we're incorporating").
//!
//! [`needs_rebuild`] says when the level the home was built at, or the installed model packs
//! (`model_packs::ui::installed_generation`), changed; the engine's half,
//! `engine::furniture_detail::before_scene`, then rebuilds the home's bodies, so a change in
//! Settings or a finished pack download shows at once.
//!
//! The GUI's (humanity-gui in scripts/crate-layers.json): Settings and the saved config read the
//! levels, so they sit below the engine, which reaches down to them.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

use crate::renderer::bounced_light_detail::{adapter_memory, mb_words, AdapterMemory};

/// Where the levels are read from (a change takes effect at the next start).
pub const LEVELS_PATH: &str = "data/gui/furniture_detail.json";
/// The level a fresh install starts on, and the fallback for an id the file does not list.
pub const DEFAULT_LEVEL: &str = "auto";

/// One choice in the list.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DetailLevel {
    pub id: String,
    /// Its name and its one-line note, by i18n key.
    pub words: String,
    pub note: String,
    /// What each model texture's side is divided by: 1 as it ships, 2 for half, 0 for no model at
    /// all (the plain box). None for Auto, which resolves to Full or Simple.
    #[serde(default)]
    pub texture_divisor: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct LevelsFile {
    /// Auto draws Full on a card reporting at least this much memory of its own (or reporting
    /// none: an integrated GPU shares the computer's memory), Simple below it.
    auto_full_min_card_bytes: u64,
    /// The share of the card's own memory past which a level is warned about.
    warn_share: f64,
    levels: Vec<DetailLevel>,
}

fn built_in() -> LevelsFile {
    LevelsFile {
        auto_full_min_card_bytes: 3 << 30,
        warn_share: 0.125,
        levels: vec![DetailLevel {
            id: "full".into(),
            words: "furniture_detail.level.full".into(),
            note: "furniture_detail.note.full".into(),
            texture_divisor: Some(1),
        }],
    }
}

fn file() -> &'static LevelsFile {
    static FILE: std::sync::OnceLock<LevelsFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| match std::fs::read_to_string(LEVELS_PATH).ok().map(|t| serde_json::from_str::<LevelsFile>(&t)) {
        Some(Ok(f)) if !f.levels.is_empty() => f,
        Some(Err(e)) => {
            log::warn!("[FurnitureDetail] {LEVELS_PATH}: {e}; offering Full only");
            built_in()
        }
        _ => {
            log::warn!("[FurnitureDetail] {LEVELS_PATH} not found; offering Full only");
            built_in()
        }
    })
}

/// Every level, in the order the file gives.
pub fn levels() -> &'static [DetailLevel] {
    &file().levels
}

pub fn is_level(id: &str) -> bool {
    levels().iter().any(|l| l.id == id)
}

/// The level `id` names, or the default's for an unknown id (the first level failing that).
pub fn level(id: &str) -> &'static DetailLevel {
    let all = levels();
    all.iter().find(|l| l.id == id).or_else(|| all.iter().find(|l| l.id == DEFAULT_LEVEL)).unwrap_or(&all[0])
}

/// What the chosen level draws: the texture divisor (1, 2, or 0 for boxes), Auto resolved from the
/// card's memory.
pub fn texture_divisor(id: &str, card: Option<&AdapterMemory>) -> u32 {
    match level(id).texture_divisor {
        Some(d) => d,
        None => {
            let small = card.filter(|a| !a.integrated).and_then(|a| a.dedicated_bytes).is_some_and(|b| b < file().auto_full_min_card_bytes);
            if small { 2 } else { 1 }
        }
    }
}

/// What the last body build's models took in video memory: their textures (one per model, shared by
/// its instances), their meshes (one per instance), and the divisor it drew at.
static MEASURED_TEXTURES: AtomicU64 = AtomicU64::new(0);
static MEASURED_MESHES: AtomicU64 = AtomicU64::new(0);
static MEASURED_DIVISOR: AtomicU64 = AtomicU64::new(0);

/// Recorded by the body build: `textures` and `meshes` bytes uploaded for the home's models at
/// `divisor`, in `took`.
pub fn note_model_memory(textures: u64, meshes: u64, divisor: u32, took: std::time::Duration) {
    MEASURED_TEXTURES.store(textures, Ordering::Relaxed);
    MEASURED_MESHES.store(meshes, Ordering::Relaxed);
    MEASURED_DIVISOR.store(divisor as u64, Ordering::Relaxed);
    // One line per body build (not per frame), so a rig's run.log carries the figures the Settings
    // page shows, and a budget check (docs/design/simready-props.md, section 9) can read them and
    // the build's time without opening the page.
    let mib = |b: u64| b as f64 / 1_048_576.0;
    log::info!(
        "[FurnitureDetail] the home's models take {:.1} MiB of textures and {:.1} MiB of meshes at divisor {divisor}; the body build took {} ms",
        mib(textures),
        mib(meshes),
        took.as_millis()
    );
}

/// Full's measured figures, (textures, meshes): the textures as measured at Full, or four times a
/// Simple build's; the meshes, which no level shrinks, as measured. None before a home with models
/// has been built (or after a Boxes build, which measures nothing).
pub fn measured_full_bytes() -> Option<(u64, u64)> {
    let textures = MEASURED_TEXTURES.load(Ordering::Relaxed);
    let meshes = MEASURED_MESHES.load(Ordering::Relaxed);
    match MEASURED_DIVISOR.load(Ordering::Relaxed) {
        1 if textures + meshes > 0 => Some((textures, meshes)),
        2 if textures + meshes > 0 => Some((textures * 4, meshes)),
        _ => None,
    }
}

/// The memory a level's models take, from Full's measured figures: the meshes, and the textures
/// divided by the divisor on each side; nothing for Boxes.
pub fn level_bytes(divisor: u32, (textures, meshes): (u64, u64)) -> u64 {
    match divisor {
        0 => 0,
        d => meshes + textures / (d as u64 * d as u64),
    }
}

/// The figure shown beside a level ("about 124 MB", "none"), or None before anything is measured.
pub fn level_memory_words(id: &str) -> Option<String> {
    let full = measured_full_bytes()?;
    let d = texture_divisor(id, adapter_memory());
    Some(if d == 0 { humanity_core::i18n::t("furniture_detail.memory_none").to_string() } else { mb_words(level_bytes(d, full)) })
}

/// The card line under the setting, and a warning when the chosen level takes more than the file's
/// share of the card's own memory: what it needs, what the card has, and what works instead.
pub fn hardware_notice(id: &str) -> (String, Option<String>) {
    use humanity_core::i18n::{t, tf};
    let card = adapter_memory();
    let d = texture_divisor(id, card);
    let Some(a) = card else { return (t("furniture_detail.card_unknown").to_string(), None) };
    if a.integrated {
        return (tf("furniture_detail.card_integrated", &[("card", &a.name)]), None);
    }
    let Some(total) = a.dedicated_bytes.filter(|b| *b > 0) else {
        return (tf("furniture_detail.card_unreported", &[("card", &a.name)]), None);
    };
    let line = tf("furniture_detail.card_line", &[("card", &a.name), ("memory", &mb_words(total))]);
    let Some(full) = measured_full_bytes() else { return (line, None) };
    let needs = level_bytes(d, full);
    let warn = (needs as f64 > total as f64 * file().warn_share).then(|| {
        tf("furniture_detail.too_much", &[("needs", &mb_words(needs)), ("card", &a.name), ("memory", &mb_words(total))])
    });
    (line, warn)
}

/// What the home's bodies were last built with: the divisor and the pack generation, packed.
static BUILT_WITH: AtomicU64 = AtomicU64::new(u64::MAX);

fn signature(divisor: u32) -> u64 {
    ((crate::model_packs::ui::installed_generation() & 0xffff_ffff) << 8) | divisor as u64
}

/// Called by the body build with the divisor it drew at.
pub fn note_built(divisor: u32) {
    BUILT_WITH.store(signature(divisor), Ordering::Relaxed);
}

/// Do the home's bodies need building again for the level `id`? True once a body build has run and
/// the level it drew at, or the installed model packs, changed since.
pub fn needs_rebuild(id: &str) -> bool {
    let built = BUILT_WITH.load(Ordering::Relaxed);
    built != u64::MAX && signature(texture_divisor(id, adapter_memory())) != built
}

/// Divide an RGBA8 image's sides by `divisor` with a box filter, for the Simple level: each
/// output texel the mean of the `divisor` x `divisor` texels under it (in the texture's own sRGB
/// values: a model texture's neighbouring texels are close, so the error of not going through linear
/// light is under a step of 8 bits for these textures). Returns the input unchanged at divisor 1.
pub fn shrink_rgba(rgba: Vec<u8>, w: u32, h: u32, divisor: u32) -> (Vec<u8>, u32, u32) {
    if divisor <= 1 || w < divisor || h < divisor {
        return (rgba, w, h);
    }
    let (nw, nh) = (w / divisor, h / divisor);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    let n = (divisor * divisor) as u32;
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0u32; 4];
            for j in 0..divisor {
                for i in 0..divisor {
                    let s = (((y * divisor + j) * w + x * divisor + i) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[s + c] as u32;
                    }
                }
            }
            let d = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[d + c] = ((acc[c] + n / 2) / n) as u8;
            }
        }
    }
    (out, nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(gb: u64) -> AdapterMemory {
        AdapterMemory { name: "test card".into(), integrated: false, dedicated_bytes: Some(gb << 30) }
    }

    /// AUTO FOLLOWS THE CARD, AND EVERY LEVEL OF THE FILE MEANS WHAT IT SAYS: Auto draws Full on a
    /// 12 GB card and on one that reports nothing, Simple on a 2 GB card; Full, Simple and Boxes
    /// draw at 1, 2 and 0. Seen red 2026-11-09 with the Auto rule's two outcomes swapped: "a 12 GB
    /// card draws Full" (it drew 2).
    #[test]
    fn auto_follows_the_card_and_the_levels_mean_what_they_say() {
        assert!(is_level("auto") && is_level("full") && is_level("simple") && is_level("boxes"), "the file offers the four levels");
        assert_eq!(texture_divisor("auto", Some(&card(12))), 1, "a 12 GB card draws Full");
        assert_eq!(texture_divisor("auto", Some(&card(2))), 2, "a 2 GB card draws Simple");
        assert_eq!(texture_divisor("auto", None), 1, "a card that reports nothing draws Full");
        assert_eq!((texture_divisor("full", None), texture_divisor("simple", None), texture_divisor("boxes", None)), (1, 2, 0));
        assert_eq!(level("no-such-level").id, DEFAULT_LEVEL, "an unknown id falls back to Auto");
    }

    /// SIMPLE TAKES A QUARTER OF THE TEXTURES: halving each side keeps the image's mean colour and
    /// makes a quarter of the texels; from the measured Full figures Simple takes the meshes and a
    /// quarter of the textures, Boxes nothing. Seen red 2026-11-09 with the meshes left out of a
    /// level's figure (the Settings page showed 72 MB for a home whose models took 140 MB:
    /// "Simple keeps the meshes": 100 against 150).
    #[test]
    fn simple_takes_a_quarter_of_the_textures() {
        let rgba: Vec<u8> = (0..(8 * 8 * 4)).map(|i| (i % 251) as u8).collect();
        let (out, w, h) = shrink_rgba(rgba.clone(), 8, 8, 2);
        assert_eq!((w, h, out.len()), (4, 4, 4 * 4 * 4));
        let mean = |v: &[u8]| v.iter().map(|&b| b as u64).sum::<u64>() / v.len() as u64;
        assert!(mean(&out).abs_diff(mean(&rgba)) <= 1, "the mean colour is kept");
        assert_eq!(level_bytes(2, (400, 50)), 150, "Simple keeps the meshes");
        assert_eq!(level_bytes(0, (400, 50)), 0, "Boxes takes nothing");
        assert_eq!(level_bytes(1, (400, 50)), 450, "Full takes both");
    }
}
