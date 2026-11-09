//! `data/models/packs.ron`, written by `just simready` (scripts/simready/pack.js): the models that
//! ship in the app and each downloadable pack with its file, size, SHA-256, licence and models.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The catalog of model packs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelPackCatalog {
    /// The GitHub release the packs are assets of (a tag that is not `v*`, so uploading triggers no
    /// build, the star catalog's way).
    pub release: String,
    /// Where the pack files are downloaded from; each pack's `file` is appended.
    pub base_url: String,
    /// What ships inside the app.
    pub core: CoreModels,
    pub packs: Vec<ModelPack>,
}

/// The models that ship inside the app, always present.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CoreModels {
    /// Its name, by key.
    pub words: String,
    /// How many models, and the bytes they take.
    pub models: u32,
    pub bytes: u64,
    /// The licence in plain words, by key.
    pub licence: String,
}

/// One downloadable pack.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelPack {
    pub id: String,
    /// Bumped when the pack's contents change; an installed pack of an older version is offered
    /// again.
    pub version: u32,
    /// Its name, by key.
    pub words: String,
    /// The archive's file name on the release.
    pub file: String,
    /// The archive's size and SHA-256 (lower-case hex).
    pub bytes: u64,
    pub sha256: String,
    /// The licence in plain words, by key.
    pub licence: String,
    /// The models it holds (their folders), so the section can say what it adds.
    pub models: Vec<String>,
}

impl ModelPack {
    /// The download address, with `base` (the catalog's, or a test server's) in front of the file.
    pub fn url(&self, base: &str) -> String {
        format!("{}{}", base, self.file)
    }

    /// Built by the pipeline (it has a size and a checksum to check a download against).
    pub fn is_built(&self) -> bool {
        self.bytes > 0 && self.sha256.len() == 64
    }

    /// The same pack in the shape the shared pack installer takes (`instrument_packs::install`,
    /// written for the instrument packs and used by both: download, check the SHA-256, unpack into
    /// a side folder, check its `pack.ron`, swap it in). Only the id, version, file, size and
    /// checksum matter to it; the instrument list is empty because a model pack holds none.
    pub fn install_entry(&self) -> crate::instrument_packs::catalog::PackEntry {
        crate::instrument_packs::catalog::PackEntry {
            id: self.id.clone(),
            version: self.version,
            family: "models".to_string(),
            words: self.words.clone(),
            file: self.file.clone(),
            bytes: self.bytes,
            sha256: self.sha256.clone(),
            licence: self.licence.clone(),
            instruments: Vec::new(),
        }
    }
}

impl ModelPackCatalog {
    /// Read `<data>/models/packs.ron`.
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("models").join("packs.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn pack(&self, id: &str) -> Option<&ModelPack> {
        self.packs.iter().find(|p| p.id == id)
    }
}

/// Where installed packs live: `<data>/models/packs/<id>/`. A catalogue row names a pack's model as
/// `models/packs/<id>/<model>/<model>.glb`, which the model loader finds under the data folder.
pub fn packs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("packs")
}
