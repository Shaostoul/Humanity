//! THE GARMENT TEXTURE PACKS (2026-11-10; docs/design/character-animation.md section 16): the
//! garment textures above the shipped 1K, as optional downloads, the model packs' way.
//!
//! The operator, 2026-11-10: "We may want to add a selector for the textures. 1k, 2k, 4k, 8k,
//! whatever." The shipped files carry each garment at 1024 texels a half and every level below; a
//! level above that (2K today) comes as one pack per size, built by the converter
//! (`scripts/gen-makehuman-look.js --pack`, BC7 by our own encoder, deterministic, `--check-pack`).
//!
//! - **The catalog is compiled in** (`include_str!` of data/characters/garment_packs.ron, which the
//!   converter writes in the same run as the archives): each pack's size and SHA-256 are the
//!   uploaded archive's, so a download is checked against what the signed release vouches for, and
//!   a mod editing the data folder cannot point the app at other files (the AI catalog's rule).
//! - **The archive** is a GitHub release asset under the non-`v*` tag `assets-garments-1`
//!   (uploading is the top-level session's step, docs/ai/release.md), downloaded only when the person
//!   picks a level that needs it, resumable (`instrument_packs::install::download_and_install`, the
//!   shared pack installer: `Range` requests, the SHA-256 checked before anything is unpacked), then
//!   installed into `<data>/characters/packs/<id>/` and removable from Settings.
//! - **A garment is in a pack only at a size its own colour map reaches**: the converter refuses to
//!   make a picture bigger than it was made, so a pack never holds an upscale labelled as more.
//!
//! **Not a cache.** An installed pack is data the person chose to install, checked against its
//! compiled-in SHA-256; it is never worked out again from anything, and removing it changes what is
//! drawn (the level falls back to the largest whose files are on this computer).

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The catalog this app was built with.
pub const BUILT_IN: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/garment_packs.ron"));

/// Overrides the catalog's download address, for trying packs served from this computer before
/// they are uploaded (`HUMANITY_GARMENT_PACKS_URL=http://127.0.0.1:8000/`). A dev aid only.
pub const URL_OVERRIDE_ENV: &str = "HUMANITY_GARMENT_PACKS_URL";

/// One garment's texture in a pack, and its DDS's SHA-256 (the converter's spot check compares it).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PackGarment {
    pub id: String,
    pub sha256: String,
}

/// One downloadable pack: every garment whose art reaches `px`, at `px` texels a half.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GarmentPack {
    pub id: String,
    /// Bumped when its contents change; an installed pack of an older version is not used.
    pub version: u32,
    /// The texels a half its garments are at.
    pub px: u32,
    /// The archive's file name on the release, its size and SHA-256 (lower-case hex).
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub garments: Vec<PackGarment>,
}

impl GarmentPack {
    /// The download address, with `base` (the catalog's, or a test server's) in front of the file.
    pub fn url(&self, base: &str) -> String {
        format!("{}{}", base, self.file)
    }

    pub fn holds(&self, garment: &str) -> bool {
        self.garments.iter().any(|g| g.id == garment)
    }

    /// The same pack in the shape the shared pack installer takes (`instrument_packs::install`).
    pub fn install_entry(&self) -> crate::instrument_packs::catalog::PackEntry {
        crate::instrument_packs::catalog::PackEntry {
            id: self.id.clone(),
            version: self.version,
            family: "garments".to_string(),
            words: "garment_textures.title".to_string(),
            file: self.file.clone(),
            bytes: self.bytes,
            sha256: self.sha256.clone(),
            licence: String::new(),
            about: None,
            credits: Vec::new(),
            instruments: Vec::new(),
            waves: Vec::new(),
        }
    }
}

/// `data/characters/garment_packs.ron`.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct GarmentPackCatalog {
    /// The GitHub release the packs are assets of (a tag that is not `v*`).
    pub release: String,
    pub base_url: String,
    pub packs: Vec<GarmentPack>,
}

impl GarmentPackCatalog {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn pack(&self, id: &str) -> Option<&GarmentPack> {
        self.packs.iter().find(|p| p.id == id)
    }

    /// The pack holding `garment` at `px` texels a half, if any.
    pub fn holding(&self, garment: &str, px: u32) -> Option<&GarmentPack> {
        self.packs.iter().find(|p| p.px == px && p.holds(garment))
    }
}

/// The compiled-in catalog (a broken one is logged and offers no pack: the levels that need one say
/// they are not published).
pub fn catalog() -> &'static GarmentPackCatalog {
    static CAT: std::sync::OnceLock<GarmentPackCatalog> = std::sync::OnceLock::new();
    CAT.get_or_init(|| {
        GarmentPackCatalog::parse(BUILT_IN).unwrap_or_else(|e| {
            log::error!("[Garments] data/characters/garment_packs.ron did not read ({e}); no garment texture pack is offered");
            GarmentPackCatalog::default()
        })
    })
}

/// The base address packs download from: the override when set (a local test server), else the
/// catalog's.
pub fn base_url(catalog: &GarmentPackCatalog) -> String {
    std::env::var(URL_OVERRIDE_ENV).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| catalog.base_url.clone())
}

/// Where installed packs live: `<data>/characters/packs/<id>/`.
pub fn packs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("characters").join("packs")
}

/// A garment's texture file in an installed pack.
pub fn garment_file(data_dir: &Path, pack: &str, garment: &str) -> PathBuf {
    packs_dir(data_dir).join(pack).join(format!("{garment}.dds"))
}
