//! GARMENT TEXTURES (2026-11-10): Settings > Graphics > Garment textures, and the hardware notice
//! beside it (docs/design/character-animation.md section 16).
//!
//! The clothes, shoes, hats, hair, brows and lashes people wear are textured, and their textures
//! live in the garment texture pool (`renderer::texture_pool`), allocated once per size. The levels
//! are data (data/gui/garment_textures.json): Full draws each half of a garment's texture at 1024
//! texels, Low memory at 512 (the converter's files carry every level, so Low memory uploads from the
//! second: a quarter of the memory). These are the dual-mode rule's realistic and simplified modes
//! for the look's textures (CLAUDE.md, "Every deep system ships two modes").
//!
//! The memory beside each level is worked out, not guessed: the engine sizes the pools from the
//! library and the near tier's cap when the characters load (`engine::animation`) and records each
//! level's bytes here ([`note_plan`]), at the format this GPU draws. Where the GPU offers no block
//! compression (wgpu's `TEXTURE_COMPRESSION_BC`) the textures are stored uncompressed, four times the
//! memory, and the page says so in words: what it needs, what the card has, what works instead
//! (operator, 2026-11-08: "We should make sure that we clearly notice and inform users when they have
//! a GPU or whatever that can't run something we're incorporating").
//!
//! The GUI's (humanity-gui in scripts/crate-layers.json): Settings and the saved config read the
//! levels, so they sit below the engine, which reaches down to them.

use serde::Deserialize;

use crate::renderer::bounced_light_detail::{adapter_memory, mb_words, AdapterMemory};

/// Where the levels are read from (a change takes effect at the next start).
pub const LEVELS_PATH: &str = "data/gui/garment_textures.json";
/// The level a fresh install starts on, and the fallback for an id the file does not list.
pub const DEFAULT_LEVEL: &str = "full";

/// One choice in the list.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TextureLevel {
    pub id: String,
    /// Its name and its one-line note, by i18n key.
    pub words: String,
    pub note: String,
    /// The most texels each half of a garment's texture is drawn at.
    pub max_px: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct LevelsFile {
    levels: Vec<TextureLevel>,
}

fn built_in() -> Vec<TextureLevel> {
    vec![TextureLevel { id: DEFAULT_LEVEL.into(), words: "garment_textures.level.full".into(), note: "garment_textures.note.full".into(), max_px: 1024 }]
}

fn parse(text: Option<&str>) -> Vec<TextureLevel> {
    match text.map(serde_json::from_str::<LevelsFile>) {
        Some(Ok(f)) if !f.levels.is_empty() => f.levels,
        Some(Err(e)) => {
            log::warn!("[Garments] {LEVELS_PATH}: {e}; offering Full only");
            built_in()
        }
        _ => {
            log::warn!("[Garments] {LEVELS_PATH} not found or empty; offering Full only");
            built_in()
        }
    }
}

/// Every level, in the order the file gives.
pub fn levels() -> &'static [TextureLevel] {
    static LEVELS: std::sync::OnceLock<Vec<TextureLevel>> = std::sync::OnceLock::new();
    LEVELS.get_or_init(|| parse(std::fs::read_to_string(LEVELS_PATH).ok().as_deref()))
}

pub fn is_level(id: &str) -> bool {
    levels().iter().any(|l| l.id == id)
}

/// The level `id` names, or the default's for an unknown id (the first level failing that).
pub fn level(id: &str) -> &'static TextureLevel {
    let all = levels();
    all.iter().find(|l| l.id == id).or_else(|| all.iter().find(|l| l.id == DEFAULT_LEVEL)).unwrap_or(&all[0])
}

/// The most texels a half the level `id` draws at.
pub fn max_px(id: &str) -> u32 {
    level(id).max_px
}

/// What the pools take at each level on this GPU (`note_plan`): (level id, bytes), and whether they
/// are BC7.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub bytes: Vec<(String, u64)>,
    pub compressed: bool,
}

static PLAN: std::sync::Mutex<Option<Plan>> = std::sync::Mutex::new(None);

/// Recorded by the engine when the characters load: each level's pool bytes on this GPU.
pub fn note_plan(plan: Plan) {
    let mib = |b: u64| b as f64 / 1_048_576.0;
    log::info!(
        "[Garments] the garment texture pools take {} ({})",
        plan.bytes.iter().map(|(id, b)| format!("{:.1} MiB at {id}", mib(*b))).collect::<Vec<_>>().join(", "),
        if plan.compressed { "BC7" } else { "uncompressed: no block compression on this GPU" }
    );
    if let Ok(mut p) = PLAN.lock() {
        *p = Some(plan);
    }
}

/// The plan, once the characters have loaded.
pub fn plan() -> Option<Plan> {
    PLAN.lock().ok().and_then(|p| p.clone())
}

/// What the own-mesh pool sets aside (2026-11-10, `renderer::mesh_pool`): each near person's own
/// body with their clothes and hair lives in it, allocated once when the world is entered, the
/// same way as the texture pools; Settings shows it under them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPlan {
    /// The pool's buffers and the shared bodies' near places, bytes.
    pub bytes: u64,
    /// How many people near it holds at the largest size at once (the near tier and the player).
    pub people: u32,
}

static MESH_PLAN: std::sync::Mutex<Option<MeshPlan>> = std::sync::Mutex::new(None);

/// Recorded by the engine when the characters load.
pub fn note_mesh_plan(plan: MeshPlan) {
    log::info!("[Garments] the own-mesh pool takes {:.1} MiB for {} people near", plan.bytes as f64 / 1_048_576.0, plan.people);
    if let Ok(mut p) = MESH_PLAN.lock() {
        *p = Some(plan);
    }
}

/// The own-mesh pool's plan, once the characters have loaded.
pub fn mesh_plan() -> Option<MeshPlan> {
    MESH_PLAN.lock().ok().and_then(|p| *p)
}

/// The line under the garment texture levels saying what the people's meshes take.
pub fn mesh_memory_words(plan: &MeshPlan) -> String {
    humanity_core::i18n::tf("garment_textures.meshes", &[("memory", &mb_words(plan.bytes)), ("people", &plan.people.to_string())])
}

/// The figure shown beside a level ("about 91 MB") in `plan`.
pub fn level_memory_words(plan: &Plan, id: &str) -> Option<String> {
    plan.bytes.iter().find(|(l, _)| l == id).map(|(_, b)| mb_words(*b))
}

/// The notice under the setting when the GPU draws the garment textures uncompressed: what they
/// need (block compression, BC7), what this card has (none), and what works instead (the same
/// pictures uncompressed, four times the memory, and Low memory to take a quarter of that). None
/// where the GPU offers block compression, or before the renderer has started. Glossary marks kept
/// (`widgets::glossary_words` draws it).
pub fn hardware_notice(block_compression: Option<bool>, card: Option<&AdapterMemory>) -> Option<String> {
    use humanity_core::i18n::{fill, t_marked};
    if block_compression != Some(false) {
        return None;
    }
    Some(match card {
        Some(a) => fill(t_marked("garment_textures.no_bc_card"), &[("card", &a.name)]),
        None => t_marked("garment_textures.no_bc").to_string(),
    })
}

/// The line under the setting where the card reads block compression (glossary marks kept).
pub fn compressed_line(card: Option<&AdapterMemory>) -> Option<String> {
    let a = card?;
    Some(humanity_core::i18n::fill(humanity_core::i18n::t_marked("garment_textures.bc_line"), &[("card", &a.name)]))
}

/// The notice for this machine (`hardware_notice` with the renderer's answer and the card's name),
/// or the compressed line where the card reads BC7.
pub fn this_machine_line() -> (Option<String>, Option<String>) {
    let bc = crate::renderer::texture_pool::block_compression();
    let card = adapter_memory();
    let notice = hardware_notice(bc, card);
    let line = if bc == Some(true) { compressed_line(card) } else { None };
    (line, notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE LEVELS MEAN WHAT THEY SAY: the file offers Full (1024) and Low memory (512), Full by
    /// default, an unknown id falls back to Full, and a broken file offers Full alone.
    #[test]
    fn the_levels_mean_what_they_say() {
        assert!(is_level("full") && is_level("low"), "the file offers both levels");
        assert_eq!((max_px("full"), max_px("low")), (1024, 512));
        assert_eq!(level("no-such-level").id, DEFAULT_LEVEL);
        assert_eq!(parse(Some("{ not json")), built_in());
        assert_eq!(parse(None)[0].max_px, 1024);
    }

    /// A GPU WITHOUT BLOCK COMPRESSION IS TOLD SO, IN WORDS: the notice names the card, what the
    /// textures need and what works instead; a GPU with block compression, or a renderer not yet
    /// started, shows none. Seen red 2026-11-10 with the notice never given once the GPU's answer is
    /// known (as main gave none): "a GPU without block compression is told so".
    #[test]
    fn a_gpu_without_block_compression_is_told_so() {
        let card = AdapterMemory { name: "Test GPU".into(), integrated: true, dedicated_bytes: None };
        let words = hardware_notice(Some(false), Some(&card)).expect("a GPU without block compression is told so");
        assert!(words.contains("Test GPU"), "the notice names the card: {words}");
        assert!(words.contains("BC7"), "the notice says what the textures need: {words}");
        assert!(words.contains("Low memory"), "the notice says what works instead: {words}");
        assert!(hardware_notice(Some(false), None).is_some(), "told so before the card has reported its name");
        assert!(hardware_notice(Some(true), Some(&card)).is_none(), "a GPU with block compression shows no notice");
        assert!(hardware_notice(None, Some(&card)).is_none(), "nothing to say before the renderer starts");
    }
}
