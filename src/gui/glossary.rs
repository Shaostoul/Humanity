//! In-app glossary / dictionary (v0.195.0).
//!
//! Loads `data/glossary.json` at app init and exposes a lookup table for
//! definitions of HumanityOS-specific and general technical terms (Ed25519,
//! Dilithium3, federation, peer-to-peer, etc.). Operator 2026-05-08:
//! "we need to assume that anyone using the app has never used an app or
//! played a video game before. They'll have no idea what ed25519 or
//! dilithium3 or tcp/ip ... means. If there's a word on the screen ...
//! they should be able to mouse over it and hold the alt key ... and get
//! the definition of that word as it is in HumanityOS' dictionary."
//!
//! Lookup is case-insensitive. Use `widgets::definition_text` to render a
//! label that shows the definition on Alt+hover. As of v0.195.0 only the
//! foundation is wired — incremental adoption across pages follows.

use std::collections::HashMap;
use std::sync::OnceLock;

/// One glossary entry. Mirrors the shape of `data/glossary.json::terms[*]`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GlossaryEntry {
    pub term: String,
    #[serde(default)]
    pub category: String,
    pub definition: String,
    #[serde(default)]
    pub link: String,
}

/// Top-level shape of glossary.json.
#[derive(Debug, Clone, serde::Deserialize)]
struct GlossaryFile {
    #[serde(default)]
    categories: HashMap<String, String>,
    terms: HashMap<String, GlossaryEntry>,
}

/// Lazily-loaded global glossary. First call to `lookup` parses the JSON
/// from `data/glossary.json` and caches the result. Subsequent calls hit
/// the cache.
static GLOSSARY: OnceLock<Glossary> = OnceLock::new();

pub struct Glossary {
    /// Lowercased term key → entry. Lookup is case-insensitive so
    /// "Ed25519", "ed25519", and "ED25519" all resolve.
    terms: HashMap<String, GlossaryEntry>,
    /// Category id → human-readable display name.
    #[allow(dead_code)]
    categories: HashMap<String, String>,
}

impl Glossary {
    /// Look up a term, case-insensitive. Returns None if the term isn't
    /// in the dictionary — caller should fall back to "no definition
    /// available" or just render plain text.
    pub fn lookup(&self, term: &str) -> Option<&GlossaryEntry> {
        self.terms.get(&term.to_lowercase())
    }

    /// Look up a single word as clicked in running text (v0.989, the
    /// Library define mode): trims surrounding punctuation and tries the
    /// bare word. Multi-word terms ("semi-major axis") stay reachable via
    /// the Dictionary search; a single click can only carry one word.
    pub fn lookup_word(&self, raw: &str) -> Option<&GlossaryEntry> {
        let w = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if w.is_empty() {
            return None;
        }
        self.lookup(w)
    }

    /// Total term count — useful for diagnostics / Settings page status.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// Every entry, sorted by display term - the Dictionary browser's
    /// backing list (v0.989).
    pub fn entries_sorted(&self) -> Vec<&GlossaryEntry> {
        let mut v: Vec<&GlossaryEntry> = self.terms.values().collect();
        v.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
        v
    }

    /// Category id -> human display name ("space" -> "Space & Astronomy").
    pub fn category_name(&self, id: &str) -> Option<&str> {
        self.categories.get(id).map(|s| s.as_str())
    }

    /// Category ids in display-name order, for the filter chips.
    pub fn category_ids(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.categories.keys().map(|s| s.as_str()).collect();
        v.sort_by_key(|id| self.categories.get(*id).cloned().unwrap_or_default());
        v
    }
}

/// Load + cache the global glossary. Called once during app init.
/// Subsequent calls are cheap. If the file is missing or malformed,
/// installs an empty glossary so lookups return None silently —
/// definition tooltips just don't appear, app still works.
pub fn install() -> &'static Glossary {
    GLOSSARY.get_or_init(|| {
        let path = crate::data_dir().join("glossary.json");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Glossary not loaded ({}): {}, definition tooltips will be blank.", path.display(), e);
                return Glossary { terms: HashMap::new(), categories: HashMap::new() };
            }
        };
        let parsed: GlossaryFile = match serde_json::from_slice(&bytes) {
            Ok(g) => g,
            Err(e) => {
                log::warn!("Glossary parse failed: {}, definition tooltips will be blank.", e);
                return Glossary { terms: HashMap::new(), categories: HashMap::new() };
            }
        };
        // Normalize keys to lowercase so lookup is case-insensitive
        // regardless of how the JSON was authored.
        let terms: HashMap<String, GlossaryEntry> = parsed.terms
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();
        log::info!("Glossary loaded: {} terms across {} categories.", terms.len(), parsed.categories.len());
        Glossary { terms, categories: parsed.categories }
    })
}

/// Get the cached glossary, initializing if needed.
pub fn glossary() -> &'static Glossary {
    install()
}
