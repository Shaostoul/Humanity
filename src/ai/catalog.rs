//! The AI catalog (docs/design/embeddable-ai.md, 6.2; rung 2, 2026-11-09): every AI file HumanityOS
//! may download, with its size and SHA-256.
//!
//! **Compiled into the app** (`include_str!` of data/ai/catalog.ron, which scripts/ai-catalog.js writes
//! from its recipe, never by hand): the signed release vouches for every hash in it, and a mod editing
//! the data folder cannot point the app at another model. Infinite-of-X holds: a model is a row of the
//! file, and the app has no list of its own.
//!
//! `HUMANITY_AI_CATALOG` (a full path to a catalog file) reads a local one instead: a developer's aid,
//! like `HUMANITY_INSTRUMENT_PACKS_URL`, for trying a new file before a release carries it. It is
//! logged loudly and never used unless set.

use std::sync::Arc;

use serde::Deserialize;

/// The catalog this app was built with.
pub const BUILT_IN: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/ai/catalog.ron"));

/// A developer's catalog file in place of the built-in one.
pub const OVERRIDE_ENV: &str = "HUMANITY_AI_CATALOG";

/// The largest file published whole: anything larger comes in parts of at most this many bytes, each
/// with its own hash. GitHub's limit is "Each file included in a release must be under 2 GiB"; this is
/// 7% under it (2026-11-11), so no published file sits a hair below the limit (Ministral 3 3B is only
/// 460,640 bytes under 2 GiB and comes in two parts). The recipe's `part_bytes`
/// (scripts/ai-catalog-recipe.json) is the same number; scripts/check-ai-catalog.js holds them equal.
pub const PART_LIMIT: u64 = 2_000_000_000;

/// One part of a split file.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Part {
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
}

/// The AI engine's zip.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct EngineEntry {
    /// The engine's version: its folder under `ai/engine/` and what its `hello` should say.
    pub version: String,
    pub platform: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub parts: Vec<Part>,
    pub sources: Vec<String>,
    /// The licence in plain words, by key, and its full text under the data folder.
    pub licence: String,
    pub licence_text: String,
    /// Its rows in data/credits.ron.
    pub credits: Vec<String>,
}

/// A small file that travels with a model (its licence text, its credit README).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Extra {
    /// Its name on the release.
    pub file: String,
    /// Its name in the model's folder.
    pub install_as: String,
    pub bytes: u64,
    pub sha256: String,
}

/// A model.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ModelEntry {
    pub id: String,
    /// The choice in data/ai/choices.ron it fills (its words and its needs live there).
    pub choice: String,
    /// "brain" (llama.cpp); "voice" and "image_reader" come with rungs 5 and 8.
    pub kind: String,
    pub model: String,
    pub maker: String,
    pub licence: String,
    pub licence_text: String,
    /// The dated finding that cleared it (docs/reference/findings/).
    pub finding: String,
    /// Its rows in data/credits.ron.
    pub credits: Vec<String>,
    pub upstream: String,
    /// The publisher's own hash of the same file (equal to `sha256` for an unchanged file).
    pub upstream_sha256: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub parts: Vec<Part>,
    #[serde(default)]
    pub extras: Vec<Extra>,
    /// The working memory, in tokens, the app asks for when it loads it.
    pub context: u32,
}

impl ModelEntry {
    /// Everything that downloads for it, in bytes (the model, or its parts, and its extras).
    pub fn download_bytes(&self) -> u64 {
        self.bytes + self.extras.iter().map(|x| x.bytes).sum::<u64>()
    }
}

/// The whole catalog.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AiCatalog {
    pub release: String,
    pub base_url: String,
    pub engine: EngineEntry,
    pub models: Vec<ModelEntry>,
}

fn is_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// A name that is a plain file name (nothing that could reach outside the folder it is written to).
pub fn plain_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\', ':']) && s != "." && s != ".." && !s.starts_with('.') && s.len() < 200
}

impl AiCatalog {
    pub fn parse(text: &str) -> Result<AiCatalog, String> {
        let c: AiCatalog = ron::from_str(text).map_err(|e| e.to_string())?;
        let problems = c.problems();
        if problems.is_empty() {
            Ok(c)
        } else {
            Err(problems.join("; "))
        }
    }

    pub fn model(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The model that fills a choice of data/ai/choices.ron, if one can be downloaded today.
    pub fn for_choice(&self, choice: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.choice == choice)
    }

    /// What the app refuses to use: a file without a real hash or size, a name that is not a plain file
    /// name, parts that do not add up to the whole or that a file of PART_LIMIT or less should not have,
    /// a part over PART_LIMIT, an id twice. The same rules as scripts/check-ai-catalog.js.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        fn file(out: &mut Vec<String>, what: &str, name: &str, bytes: u64, sha: &str, parts: &[Part]) {
            if !plain_name(name) {
                out.push(format!("{what}: {name:?} is not a plain file name"));
            }
            if bytes == 0 {
                out.push(format!("{what}: no size"));
            }
            if !is_hash(sha) {
                out.push(format!("{what}: {sha:?} is not a SHA-256"));
            }
            if bytes > PART_LIMIT && parts.is_empty() {
                out.push(format!("{what}: {bytes} bytes is over {PART_LIMIT}, so it must come in parts"));
            }
            if bytes <= PART_LIMIT && !parts.is_empty() {
                out.push(format!("{what}: {PART_LIMIT} bytes or smaller but listed in parts"));
            }
            if !parts.is_empty() && parts.iter().map(|p| p.bytes).sum::<u64>() != bytes {
                out.push(format!("{what}: its parts do not add up to its {bytes} bytes"));
            }
            for p in parts {
                if !plain_name(&p.file) || p.bytes == 0 || p.bytes > PART_LIMIT || !is_hash(&p.sha256) {
                    out.push(format!("{what}: the part {:?} is not a plain name with a size of at most {PART_LIMIT} bytes and a SHA-256", p.file));
                }
            }
        }
        file(&mut out, "the engine", &self.engine.file, self.engine.bytes, &self.engine.sha256, &self.engine.parts);
        let mut ids = std::collections::BTreeSet::new();
        for m in &self.models {
            file(&mut out, &m.id, &m.file, m.bytes, &m.sha256, &m.parts);
            for x in &m.extras {
                file(&mut out, &format!("{}'s {}", m.id, x.file), &x.file, x.bytes, &x.sha256, &[]);
                if !plain_name(&x.install_as) {
                    out.push(format!("{}: {:?} is not a plain file name", m.id, x.install_as));
                }
            }
            if !ids.insert(m.id.clone()) {
                out.push(format!("the model {} twice", m.id));
            }
            if !plain_name(&m.id) {
                out.push(format!("the model id {:?} is not a plain name", m.id));
            }
            if m.kind != "brain" {
                out.push(format!("{}: the engine runs only brains in this version, not {:?}", m.id, m.kind));
            }
        }
        if !plain_name(&self.engine.version) {
            out.push(format!("the engine version {:?} is not a plain name", self.engine.version));
        }
        // https only, but for a server on this computer (a developer's catalog through
        // HUMANITY_AI_CATALOG serving files before they are uploaded; scripts/serve-ai-files.js).
        let loopback = self.base_url.starts_with("http://127.0.0.1:") || self.base_url.starts_with("http://localhost:");
        if !(self.base_url.starts_with("https://") || loopback) || !self.base_url.ends_with('/') {
            out.push(format!("the base address {:?} is not https://.../", self.base_url));
        }
        out
    }
}

/// The catalog to use now: the built-in one, or the developer's when HUMANITY_AI_CATALOG names one.
/// Parsed on each call (it is small); Err when it does not read or breaks a rule, which Settings > AI
/// says in words instead of offering anything.
pub fn current() -> Result<Arc<AiCatalog>, String> {
    if let Some(path) = std::env::var_os(OVERRIDE_ENV) {
        let p = std::path::PathBuf::from(&path);
        if !p.is_absolute() {
            return Err(format!("{OVERRIDE_ENV} must be a full path, not {}", p.display()));
        }
        log::warn!("[AI] using the developer's catalog {} instead of the built-in one ({OVERRIDE_ENV})", p.display());
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        return AiCatalog::parse(&text).map(Arc::new).map_err(|e| format!("{}: {e}", p.display()));
    }
    AiCatalog::parse(BUILT_IN).map(Arc::new)
}

/// The download address of a file, `base` (the catalog's, or a developer's server) in front of it.
pub fn url(base: &str, file: &str) -> String {
    format!("{base}{file}")
}
