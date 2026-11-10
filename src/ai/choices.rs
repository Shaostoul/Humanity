//! What each later AI choice will need (docs/design/embeddable-ai.md, 4.1, 4.7 and 9.1), from
//! data/ai/choices.ron: the features (talking characters, the helper, the computer voice), each choice
//! of model for them, and its needs. Infinite-of-X: a choice is a row there, never code. Rung 2's catalog
//! (data/ai/catalog.ron, compiled in, with each file's hash) names the same ids.
//!
//! Read again every few seconds while Settings > AI is open, and every time with the Dev page's "turn
//! off all reuse" switch off (CLAUDE.md's reuse rule), so an edit to the file shows without a restart.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The file, under the data folder.
pub const PATH: &str = "ai/choices.ron";

/// How long a reading of the file is used before it is read again (with reuse on).
const REREAD: Duration = Duration::from_secs(5);

/// A feature AI can add, and what it is without AI (5.1's table).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Feature {
    pub id: String,
    /// Words keys: its name, what it does with AI, and what happens without it.
    pub name: String,
    pub about: String,
    pub without: String,
}

/// What a choice needs while the person plays (estimates until rung 4 measures them).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Needs {
    /// Free memory while running, MB (the model, its working memory and buffers: 4.1's column).
    pub memory_mb: u64,
    /// Processor cores the computer should have.
    pub cores: u32,
    /// Free space for its files, MB.
    pub disk_mb: u64,
    /// It runs much faster with AVX2.
    pub avx2: bool,
    /// The graphics card's own memory it needs outside the 3D world (the helper's route, 4.4), MB.
    #[serde(default)]
    pub vram_mb: Option<u64>,
}

/// One choice of model for a feature.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Choice {
    pub id: String,
    /// The feature's id.
    pub feature: String,
    /// Words key of its name ("Talking characters (Rich)").
    pub words: String,
    /// The model's own name and its maker, as they publish them (names, the same in every language).
    pub model: String,
    pub maker: String,
    pub needs: Needs,
    /// The lighter choice to offer instead when this one does not fit.
    #[serde(default)]
    pub fallback: Option<String>,
    /// The rung of the design that brings it (for the Dev page and the docs).
    pub rung: u32,
}

/// The whole file.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AiChoices {
    pub features: Vec<Feature>,
    pub choices: Vec<Choice>,
}

impl AiChoices {
    pub fn feature(&self, id: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn choice(&self, id: &str) -> Option<&Choice> {
        self.choices.iter().find(|c| c.id == id)
    }

    /// The file's own consistency: every choice names a feature, every fallback a choice of the same
    /// feature that needs less memory, and no id twice.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for f in &self.features {
            if !ids.insert(format!("feature:{}", f.id)) {
                out.push(format!("feature {} twice", f.id));
            }
        }
        for c in &self.choices {
            if !ids.insert(format!("choice:{}", c.id)) {
                out.push(format!("choice {} twice", c.id));
            }
            if self.feature(&c.feature).is_none() {
                out.push(format!("{} names the feature {}, which the file does not list", c.id, c.feature));
            }
            if let Some(fb) = &c.fallback {
                match self.choice(fb) {
                    None => out.push(format!("{}'s fallback {fb} is not a choice", c.id)),
                    Some(f) if f.feature != c.feature => out.push(format!("{}'s fallback {fb} is for another feature", c.id)),
                    Some(f) if f.needs.memory_mb >= c.needs.memory_mb => out.push(format!("{}'s fallback {fb} needs no less memory", c.id)),
                    _ => {}
                }
            }
        }
        out
    }
}

/// Read and check the file.
pub fn load(data_dir: &std::path::Path) -> Result<AiChoices, String> {
    let path = data_dir.join(PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let parsed: AiChoices = ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let problems = parsed.problems();
    if !problems.is_empty() {
        return Err(format!("{}: {}", path.display(), problems.join("; ")));
    }
    Ok(parsed)
}

/// The file as Settings > AI shows it: read again after [`REREAD`], or every call with reuse off.
pub fn current() -> Result<Arc<AiChoices>, String> {
    static LAST: Mutex<Option<(Instant, Result<Arc<AiChoices>, String>)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    let fresh = humanity_core::reuse::enabled() && last.as_ref().is_some_and(|(at, _)| at.elapsed() < REREAD);
    if !fresh {
        *last = Some((Instant::now(), load(&humanity_core::storage::data_dir()).map(Arc::new)));
    }
    last.as_ref().map(|(_, r)| r.clone()).unwrap_or_else(|| Err("not read".into()))
}
