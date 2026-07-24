//! Reaction emoji palette registry: the data-driven source of truth for which
//! emoji can be used as message reactions, loaded from `data/reactions.json`
//! (compile-time embedded, mirroring the `lock_types` / `wall_materials`
//! registry pattern). Pure std + serde_json, UNGATED, because it has two very
//! different consumers that must agree:
//!
//! - the native chat page's reaction picker (quick row = `top`, full = `all`)
//! - the relay's server-side reaction allowlist (`is_allowed` / `normalize`)
//!
//! Before v0.931 these were three divergent hardcoded lists (native ~45, relay
//! 8 WITH the U+FE0F variation selector, web 8 different again), so the relay
//! silently dropped most reactions the native picker offered - including the
//! plain heart, because native strips FE0F (egui renders it as tofu) while the
//! relay's list carried "\u{2764}\u{FE0F}". The fix is one canonical BARE form
//! everywhere: entries are stored bare in the JSON, `normalize` strips FE0F
//! from any incoming string, and the relay stores/broadcasts the normalized
//! form so every client sees the same bytes.

/// Strip the U+FE0F emoji-presentation selector: the canonical stored form.
pub fn normalize(emoji: &str) -> String {
    emoji.chars().filter(|c| *c != '\u{FE0F}').collect()
}

struct Palette {
    top: Vec<String>,
    all: Vec<String>,
}

fn palette() -> &'static Palette {
    static REG: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../data/reactions.json");
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(SRC);
        match parsed {
            Ok(v) => {
                let arr = |key: &str| -> Vec<String> {
                    v.get(key)
                        .and_then(|a| a.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| e.as_str())
                                .map(normalize)
                                .filter(|s| !s.is_empty())
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let top = arr("top");
                let all = arr("all");
                if top.is_empty() || all.is_empty() {
                    log::error!("data/reactions.json missing top/all arrays; using fallback palette");
                    fallback()
                } else {
                    Palette { top, all }
                }
            }
            Err(e) => {
                log::error!("data/reactions.json parse error ({e}); using fallback palette");
                fallback()
            }
        }
    })
}

/// Sensible-default fallback if the embedded JSON is ever malformed (the
/// infinite-of-x migration template's escape hatch, never the source of truth).
fn fallback() -> Palette {
    let base = ["\u{2764}", "\u{1F44D}", "\u{1F44E}", "\u{1F602}", "\u{1F525}"];
    Palette {
        top: base.iter().map(|s| s.to_string()).collect(),
        all: base.iter().map(|s| s.to_string()).collect(),
    }
}

/// The quick-row reactions (native chat's top row; web's picker).
pub fn top() -> &'static [String] {
    &palette().top
}

/// Every allowed reaction (native full picker; the relay allowlist).
pub fn all() -> &'static [String] {
    &palette().all
}

/// Server-side check: is this emoji (in any FE0F form) an allowed reaction?
pub fn is_allowed(emoji: &str) -> bool {
    let n = normalize(emoji);
    all().iter().any(|e| *e == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_loads_from_data_file_not_fallback() {
        // The embedded JSON must parse and be a superset of the old relay
        // allowlist (minus FE0F) so no previously-valid reaction breaks.
        assert!(top().len() >= 8, "top row lost entries: {:?}", top());
        assert!(all().len() > 30, "full palette lost entries: {:?}", all().len());
        for old in ["\u{2764}\u{FE0F}", "\u{1F602}", "\u{1F44D}", "\u{1F44E}", "\u{1F525}", "\u{1F62E}", "\u{1F622}", "\u{1F389}"] {
            assert!(is_allowed(old), "previously-allowed reaction now rejected: {old}");
        }
    }

    #[test]
    fn normalize_strips_the_variation_selector() {
        assert_eq!(normalize("\u{2764}\u{FE0F}"), "\u{2764}");
        assert_eq!(normalize("\u{2764}"), "\u{2764}");
        assert!(is_allowed("\u{2764}"), "the bare native heart must be allowed");
        assert!(is_allowed("\u{2764}\u{FE0F}"), "the web FE0F heart must be allowed");
    }

    #[test]
    fn every_stored_entry_is_already_bare() {
        for e in all() {
            assert!(!e.contains('\u{FE0F}'), "entry {e:?} carries FE0F; store bare forms");
        }
    }
}
