//! GARMENT TEXTURES (2026-11-10): Settings > Graphics > Garment textures, its levels, its plan of
//! the garment texture pools, its optional packs and the hardware notices beside it
//! (docs/design/character-animation.md section 16; docs/design/graphics-settings.md).
//!
//! The clothes, shoes, hats, hair, brows and lashes people wear are textured, and their textures
//! live in the garment texture pools (`renderer::texture_pool`), allocated once per shape when the
//! world is entered.
//!
//! **The levels are data** (data/gui/garment_textures.json; the operator, 2026-11-10: "We may want
//! to add a selector for the textures. 1k, 2k, 4k, 8k, whatever"): each names the most texels each
//! half of a garment's texture is drawn at. A garment is never drawn larger than its own colour map
//! was made (`GarmentArt::art_px`, measured by the converter): at a level above it the garment is
//! drawn at its own largest, and the page says how many garments that is. **A level no garment's art
//! reaches is not offered** (CLAUDE.md, "Every graphics setting says what it actually changes": never
//! an upscale labelled as more); today that is 4K, because no MakeHuman colour map is 4096. The
//! shipped files carry 1024 a half and every level below it; a level above needs that size's pack
//! ([`packs`]), downloaded when the person picks the level, and until it is installed the largest
//! level whose files are on this computer is drawn ([`effective`]).
//!
//! **The memory beside each level is worked out, not guessed** ([`plan_for`]): the pools' shapes
//! and capacities from the garments and the near tier's cap ([`pool_shapes`], the very function the
//! engine allocates the pools with), at the format this GPU draws (BC7 where it offers block
//! compression, four times as much uncompressed where it does not). A level whose pools would take
//! more than `warn_share` of the card's own memory says so in words: what it needs, what the card
//! has, what works instead (the hardware-notice rule, CLAUDE.md).
//!
//! The GUI's (humanity-gui in scripts/crate-layers.json): Settings and the saved config read the
//! levels, so they sit below the engine, which reaches down to them. The garments' sizes come from
//! the engine ([`note_garments`]: it reads data/characters/garments.ron with the animation crate's
//! reader before the world is entered, and again when the library is read), since this layer may not
//! reach the animation crate.

pub mod packs;

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::instrument_packs::install::{self, InstalledPack, ProgressHandle, Stage};
use crate::renderer::bounced_light_detail::{adapter_memory, mb_words, AdapterMemory};
use crate::renderer::texture_pool::SlotClass;

/// Where the levels are read from (a change takes effect at the next start).
pub const LEVELS_PATH: &str = "data/gui/garment_textures.json";
/// The level a fresh install starts on, and the fallback for an id the file does not list.
pub const DEFAULT_LEVEL: &str = "1k";

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

fn default_warn_share() -> f64 {
    0.125
}

#[derive(Debug, Clone, Deserialize)]
struct LevelsFile {
    #[serde(default = "default_warn_share")]
    warn_share: f64,
    levels: Vec<TextureLevel>,
}

fn built_in() -> LevelsFile {
    LevelsFile { warn_share: default_warn_share(), levels: vec![TextureLevel { id: DEFAULT_LEVEL.into(), words: "garment_textures.level.1k".into(), note: "garment_textures.note.1k".into(), max_px: 1024 }] }
}

fn parse(text: Option<&str>) -> LevelsFile {
    match text.map(serde_json::from_str::<LevelsFile>) {
        Some(Ok(f)) if !f.levels.is_empty() => f,
        Some(Err(e)) => {
            log::warn!("[Garments] {LEVELS_PATH}: {e}; offering 1K only");
            built_in()
        }
        _ => {
            log::warn!("[Garments] {LEVELS_PATH} not found or empty; offering 1K only");
            built_in()
        }
    }
}

fn file() -> &'static LevelsFile {
    static FILE: std::sync::OnceLock<LevelsFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| parse(std::fs::read_to_string(LEVELS_PATH).ok().as_deref()))
}

/// Every level the file lists, in its order (offered or not: [`Plan::levels`] holds the offered).
pub fn levels() -> &'static [TextureLevel] {
    &file().levels
}

/// A level whose pools take more than this share of the card's own memory is warned about.
pub fn warn_share() -> f64 {
    file().warn_share
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

// ── The plan: what each level draws and what it takes ──────────────────────

/// A garment as the plan sees it: its id, its kind (a pool holds a near tier's worth of each kind of
/// its shape), the size its shipped file carries and its art's size.
#[derive(Debug, Clone, PartialEq)]
pub struct GarmentArt {
    pub id: String,
    pub kind: String,
    /// Each half's texels in the shipped file (`garments.ron`'s `texture_px`).
    pub shipped_px: u32,
    /// Its colour map's own size (`art_px`): the most it is ever drawn at.
    pub art_px: u32,
}

/// The texels a half `g` is drawn at, at a level of `level_px`: the level's size, never above its
/// art (a picture is never made bigger than it was made).
pub fn drawn_px(g: &GarmentArt, level_px: u32) -> u32 {
    level_px.min(g.art_px.max(1))
}

/// Every pool shape the garments fall in at `level_px` and its capacity: enough slots for the near
/// tier (its cap and the player) each wearing one garment of every kind that comes in that shape,
/// never more than the garments of that shape. In the order the garments first reach each shape.
/// The engine allocates the pools with exactly this ([`crate::engine`]'s `allocate_pools`), so
/// every garment finds the pool planned for it and none is allocated on the frame.
pub fn pool_shapes(garments: &[GarmentArt], near_cap: usize, level_px: u32, compressed: bool) -> Vec<(SlotClass, u32)> {
    let mut shapes: Vec<(SlotClass, Vec<&str>, usize)> = Vec::new();
    for g in garments {
        let class = SlotClass::for_half(drawn_px(g, level_px), compressed);
        match shapes.iter_mut().find(|(c, _, _)| *c == class) {
            Some((_, kinds, n)) => {
                *n += 1;
                if !kinds.contains(&g.kind.as_str()) {
                    kinds.push(&g.kind);
                }
            }
            None => shapes.push((class, vec![g.kind.as_str()], 1)),
        }
    }
    shapes.into_iter().map(|(c, kinds, n)| (c, n.min((near_cap + 1) * kinds.len()).max(1) as u32)).collect()
}

/// The bytes the pools of `shapes` take on the GPU.
pub fn pools_bytes(shapes: &[(SlotClass, u32)]) -> u64 {
    shapes.iter().map(|(c, n)| c.slot_bytes() * *n as u64).sum()
}

/// Where a garment's texture comes from at a level.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Its shipped file, its levels past `max_px` skipped.
    Shipped { max_px: u32 },
    /// Its file in pack `pack`, at `px` texels a half.
    Pack { pack: String, px: u32 },
}

/// Where `g`'s texture comes from at a level of `level_px`: the shipped file at or below its size,
/// else the pack of the size it is drawn at (its shipped file's own size when no pack holds it,
/// which [`effective`] never lets happen: a level whose packs are missing is not drawn).
pub fn source(g: &GarmentArt, level_px: u32, catalog: &packs::GarmentPackCatalog) -> Source {
    let d = drawn_px(g, level_px);
    if d <= g.shipped_px {
        return Source::Shipped { max_px: d };
    }
    match catalog.holding(&g.id, d) {
        Some(p) => Source::Pack { pack: p.id.clone(), px: d },
        None => Source::Shipped { max_px: g.shipped_px },
    }
}

/// One offered level on this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelPlan {
    pub id: String,
    pub px: u32,
    /// How many garments' art reaches it (drawn at the level's size).
    pub reaching: usize,
    /// The garments drawn at their own largest, smaller than the level: (their size, how many),
    /// largest first.
    pub below: Vec<(u32, usize)>,
    /// What its pools take on this GPU.
    pub bytes: u64,
    /// The packs its files need (none for the shipped sizes), and their download's bytes.
    pub packs: Vec<String>,
    pub download: u64,
    /// False when a file it needs is in no published pack (it is shown, saying so, and not drawn).
    pub published: bool,
}

/// Each offered level's plan, on a GPU that does (`compressed`) or does not take BC7.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub levels: Vec<LevelPlan>,
    pub compressed: bool,
}

impl Plan {
    pub fn level(&self, id: &str) -> Option<&LevelPlan> {
        self.levels.iter().find(|l| l.id == id)
    }
}

/// The plan: every level some garment's art reaches, in the file's order, with what it draws, what
/// its pools take, and the packs it needs. A level no garment's art reaches is left out.
pub fn plan_for(levels: &[TextureLevel], garments: &[GarmentArt], near_cap: usize, compressed: bool, catalog: &packs::GarmentPackCatalog) -> Plan {
    let mut out = Vec::new();
    for l in levels {
        let reaching = garments.iter().filter(|g| g.art_px >= l.max_px).count();
        if reaching == 0 {
            continue;
        }
        let mut below: BTreeMap<u32, usize> = BTreeMap::new();
        for g in garments.iter().filter(|g| g.art_px < l.max_px) {
            *below.entry(drawn_px(g, l.max_px)).or_default() += 1;
        }
        let mut need: Vec<String> = Vec::new();
        let mut published = true;
        for g in garments {
            let d = drawn_px(g, l.max_px);
            if d <= g.shipped_px {
                continue;
            }
            match catalog.holding(&g.id, d) {
                Some(p) if !need.contains(&p.id) => need.push(p.id.clone()),
                Some(_) => {}
                None => published = false,
            }
        }
        let download = need.iter().filter_map(|id| catalog.pack(id)).map(|p| p.bytes).sum();
        out.push(LevelPlan {
            id: l.id.clone(),
            px: l.max_px,
            reaching,
            below: below.into_iter().rev().collect(),
            bytes: pools_bytes(&pool_shapes(garments, near_cap, l.max_px, compressed)),
            packs: need,
            download,
            published,
        });
    }
    Plan { levels: out, compressed }
}

/// The level drawn now: the chosen one when it is offered, published and its packs are installed;
/// else the largest offered level at or below its size whose files are all on this computer; else
/// the smallest offered. None only when nothing is offered.
pub fn effective<'a>(plan: &'a Plan, chosen: &str, installed: impl Fn(&str) -> bool) -> Option<&'a LevelPlan> {
    let want = plan.level(chosen).map_or_else(|| max_px(chosen), |l| l.px);
    let ready = |l: &&LevelPlan| l.published && l.packs.iter().all(|p| installed(p));
    plan.levels.iter().filter(|l| l.px <= want).filter(ready).max_by_key(|l| l.px).or_else(|| plan.levels.iter().min_by_key(|l| l.px))
}

// ── The section's state: the garments' sizes, the packs, the downloads ──────

/// What the section shows and asks for (`shared`), and what the engine reads to know which packs are
/// installed. Kept here rather than on `GuiState` for the model packs' reason (`src/gui/mod.rs` sits at
/// its line budget).
#[derive(Debug, Default)]
pub struct GarmentTexturesUi {
    /// The garments' sizes and the near tier's cap, noted by the engine ([`note_garments`]).
    pub garments: Vec<GarmentArt>,
    pub near_cap: usize,
    pub noted_at: Option<std::time::Instant>,
    /// Why the garments could not be read, when they could not.
    pub read_error: Option<String>,
    /// The installed packs (at any version) and the bytes of downloads waiting to resume, read from
    /// `<data>/characters/packs/` (again after every install and removal, and every 2 s with the
    /// reuse switch off).
    pub installed: BTreeMap<String, InstalledPack>,
    pub partial: BTreeMap<String, u64>,
    pub read_at: Option<std::time::Instant>,
    /// The pack downloading now, and its progress (kept after a failure until the next try).
    pub download: Option<(String, ProgressHandle)>,
    /// The failed download's leftovers have been read since it failed (once, not every frame).
    pub failure_read: bool,
    /// Requests from the section, acted on by [`tick`].
    pub request_download: Option<String>,
    pub request_remove: Option<String>,
    /// The last removal that failed: (pack, why).
    pub remove_error: Option<(String, String)>,
}

impl GarmentTexturesUi {
    /// Is `id` installed at the catalog's version?
    pub fn is_installed(&self, id: &str) -> bool {
        match (self.installed.get(id), packs::catalog().pack(id)) {
            (Some(i), Some(p)) => i.version == p.version,
            _ => false,
        }
    }

    /// Is a download moving now (not finished, not failed)?
    pub fn downloading(&self) -> bool {
        self.download.as_ref().and_then(|(_, p)| p.lock().ok().map(|g| !matches!(g.stage, Stage::Done | Stage::Failed(_)))).unwrap_or(false)
    }

    /// The plan for this machine, once the garments are noted.
    pub fn plan(&self, compressed: bool) -> Option<Plan> {
        (!self.garments.is_empty()).then(|| plan_for(levels(), &self.garments, self.near_cap, compressed, packs::catalog()))
    }

    fn reload(&mut self, data_dir: &Path) {
        let dir = packs::packs_dir(data_dir);
        self.installed = install::installed_packs(&dir);
        self.partial = packs::catalog().packs.iter().map(|p| (p.id.clone(), install::partial_bytes(&dir, &p.id))).filter(|(_, b)| *b > 0).collect();
        self.read_at = Some(std::time::Instant::now());
    }
}

/// The section's state for this run (a poisoned lock is taken over rather than failing forever).
pub fn shared() -> std::sync::MutexGuard<'static, GarmentTexturesUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<GarmentTexturesUi>> = std::sync::OnceLock::new();
    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// With the reuse switch off, what this section read from disk is read again this often (seconds).
pub const REUSE_OFF_REREAD_S: f32 = 2.0;

/// Whether the engine should note the garments again: never noted, or (reuse off) due again.
pub fn garments_due() -> bool {
    let s = shared();
    s.noted_at.is_none_or(|t| !humanity_core::reuse::enabled() && t.elapsed().as_secs_f32() > REUSE_OFF_REREAD_S)
}

/// Recorded by the engine: the garments' sizes and the near tier's cap (or why they did not read).
pub fn note_garments(read: Result<(Vec<GarmentArt>, usize), String>) {
    let mut s = shared();
    s.noted_at = Some(std::time::Instant::now());
    match read {
        Ok((garments, near_cap)) => {
            s.garments = garments;
            s.near_cap = near_cap;
            s.read_error = None;
        }
        Err(e) => {
            log::warn!("[Garments] the garments' sizes did not read ({e}); Settings shows no plan");
            s.read_error = Some(e);
        }
    }
}

/// Read the installed packs now if they have never been read (the engine, before it plans the first
/// frame's pools: the frame can draw bodies before the main loop's `tick` has run once, and planning
/// at 1K then finding the 2K pack installed allocated every pool twice at world entry; seen in the
/// 2026-11-10 crowd run's log: the 1K pools, then the 2K pools a frame later).
pub fn ensure_read(data_dir: &Path) {
    let mut s = shared();
    if s.read_at.is_none() {
        s.reload(data_dir);
    }
}

/// Bumped each time a pack install or removal finishes (the engine re-plans the level it draws).
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn installed_generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

/// The frame step: read the installed packs when needed, start a requested download on its own
/// thread, remove a requested pack, notice a finished download.
pub fn tick(data_dir: &Path) {
    let mut s = shared();
    let due = s.read_at.is_none_or(|t| !humanity_core::reuse::enabled() && t.elapsed().as_secs_f32() > REUSE_OFF_REREAD_S);
    if due {
        s.reload(data_dir);
    }
    if let Some(id) = s.request_download.take() {
        if !s.downloading() {
            if let Some(pack) = packs::catalog().pack(&id).cloned() {
                let progress = install::new_progress();
                s.download = Some((id.clone(), progress.clone()));
                s.failure_read = false;
                let url = pack.url(&packs::base_url(packs::catalog()));
                let dir = packs::packs_dir(data_dir);
                let entry = pack.install_entry();
                log::info!("[Garments] downloading the {} garment textures from {url} ({} bytes{})", pack.id, pack.bytes, s.partial.get(&id).map_or(String::new(), |b| format!(", resuming after {b}")));
                let spawned = std::thread::Builder::new().name("garment-pack".into()).spawn(move || match install::download_and_install(&url, &entry, &dir, &progress) {
                    Ok(()) => log::info!("[Garments] the {} garment textures are installed", entry.id),
                    Err(f) => log::warn!("[Garments] the {} garment textures did not install: {f:?}", entry.id),
                });
                if let Err(e) = spawned {
                    log::warn!("[Garments] could not start the download ({e})");
                    s.download = None;
                }
            }
        }
    }
    if let Some(id) = s.request_remove.take() {
        match install::remove_pack(&packs::packs_dir(data_dir), &id) {
            Ok(()) => {
                s.remove_error = None;
                log::info!("[Garments] the {id} garment textures were removed");
            }
            Err(e) => s.remove_error = Some((id, e)),
        }
        GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        s.reload(data_dir);
    }
    let stage = s.download.as_ref().and_then(|(_, p)| p.lock().ok().map(|g| g.stage.clone()));
    match stage {
        Some(Stage::Done) => {
            s.download = None;
            s.failure_read = false;
            GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            s.reload(data_dir);
        }
        // A failure stays on show (and what arrived stays on disk to resume): read the partial once.
        Some(Stage::Failed(_)) if !s.failure_read => {
            s.failure_read = true;
            s.reload(data_dir);
        }
        _ => {}
    }
}

// ── The words ────────────────────────────────────────────────────────────────

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

/// A level's size in words ("2K", "512"): its own name.
fn level_name(id: &str) -> String {
    humanity_core::i18n::t_dyn(&level(id).words).to_string()
}

/// The words for a texel size in the level names' style ("1K", "512").
fn px_name(px: u32) -> String {
    match levels().iter().find(|l| l.max_px == px) {
        Some(l) => humanity_core::i18n::t_dyn(&l.words).to_string(),
        None if px % 1024 == 0 => format!("{}K", px / 1024),
        None => px.to_string(),
    }
}

/// The line saying how many garments are drawn at their own largest at the level `lp` (None when
/// every garment reaches it).
pub fn below_words(lp: &LevelPlan) -> Option<String> {
    if lp.below.is_empty() {
        return None;
    }
    let count: usize = lp.below.iter().map(|(_, n)| n).sum();
    let list = lp.below.iter().map(|(px, n)| humanity_core::i18n::tf("garment_textures.below_part", &[("count", &n.to_string()), ("size", &px_name(*px))])).collect::<Vec<_>>().join(", ");
    Some(humanity_core::i18n::tf("garment_textures.below", &[("level", &level_name(&lp.id)), ("count", &count.to_string()), ("list", &list)]))
}

/// The memory notice for the chosen level: what its pools need against the card's own memory, with
/// the level that fits as what works instead. None where it fits, or where the card reports no
/// memory of its own to measure against (the card line says which).
pub fn memory_notice(plan: &Plan, chosen: &str, card: Option<&AdapterMemory>) -> Option<String> {
    let a = card?;
    let total = a.dedicated_bytes.filter(|b| *b > 0 && !a.integrated)?;
    let lp = plan.level(chosen)?;
    let limit = total as f64 * warn_share();
    if lp.bytes as f64 <= limit {
        return None;
    }
    let fits = plan.levels.iter().filter(|l| l.bytes as f64 <= limit).max_by_key(|l| l.px);
    let instead = match fits {
        Some(l) => humanity_core::i18n::tf("garment_textures.too_much_instead", &[("level", &level_name(&l.id)), ("memory", &mb_words(l.bytes))]),
        None => humanity_core::i18n::t("garment_textures.too_much_none").to_string(),
    };
    Some(humanity_core::i18n::tf(
        "garment_textures.too_much",
        &[("level", &level_name(&lp.id)), ("needs", &mb_words(lp.bytes)), ("share", &format!("{}", (warn_share() * 1000.0).round() / 10.0)), ("card", &a.name), ("memory", &mb_words(total)), ("instead", &instead)],
    ))
}

/// The notice under the setting when the GPU draws the garment textures uncompressed: what they
/// need (block compression, BC7), what this card has (none), and what works instead (the same
/// pictures uncompressed, four times the memory, and a smaller level to take a quarter of that).
/// None where the GPU offers block compression, or before the renderer has started. Glossary marks
/// kept (`widgets::glossary_words` draws it).
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

/// The card's memory line: its own memory, or that it shares the computer's, or has not said.
pub fn card_line(card: Option<&AdapterMemory>) -> String {
    use humanity_core::i18n::{t, tf};
    let Some(a) = card else { return t("furniture_detail.card_unknown").to_string() };
    if a.integrated {
        return tf("furniture_detail.card_integrated", &[("card", &a.name)]);
    }
    match a.dedicated_bytes.filter(|b| *b > 0) {
        Some(total) => tf("furniture_detail.card_line", &[("card", &a.name), ("memory", &mb_words(total))]),
        None => tf("furniture_detail.card_unreported", &[("card", &a.name)]),
    }
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
mod tests;
