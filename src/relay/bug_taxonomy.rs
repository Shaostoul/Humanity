//! The bug reporter's severities and categories, read from `data/bugs/taxonomy.json` (built in),
//! the one list the desktop Bugs page, the website's Bugs page and this relay share (2026-10-24).
//!
//! Until then `api::create_bug` kept lists of its own (page areas such as chat and tasks, and
//! lowercase severities) while the desktop offered the taxonomy's UI, Gameplay, Network and so on:
//! every desktop report's category was quietly stored as "other". Now a value is kept when its
//! lower-case form is an id on the list, and replaced by the fallback otherwise.

use std::sync::OnceLock;

/// The built-in copy of `data/bugs/taxonomy.json`: a relay checks reports against the list it
/// was built with, so an edit on a server's disk never changes what it accepts.
const TAXONOMY_JSON: &str = include_str!("../../data/bugs/taxonomy.json");

#[derive(serde::Deserialize)]
struct Taxonomy {
    #[serde(default)]
    severities: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
}

/// `(severity ids, category ids)`: the taxonomy's words in lower case.
fn ids() -> &'static (Vec<String>, Vec<String>) {
    static IDS: OnceLock<(Vec<String>, Vec<String>)> = OnceLock::new();
    IDS.get_or_init(|| {
        let t: Taxonomy = serde_json::from_str(TAXONOMY_JSON).expect("data/bugs/taxonomy.json parses");
        let lower = |v: Vec<String>| v.into_iter().map(|s| s.trim().to_lowercase()).collect();
        (lower(t.severities), lower(t.categories))
    })
}

fn keep_or(value: &str, allowed: &[String], fallback: &str) -> String {
    let v = value.trim().to_lowercase();
    if allowed.iter().any(|a| *a == v) { v } else { fallback.to_string() }
}

/// The severity a report is stored with: its id when on the list, otherwise "medium".
pub fn severity(value: &str) -> String {
    keep_or(value, &ids().0, "medium")
}

/// The category a report is stored with: its id when on the list, otherwise "other".
pub fn category(value: &str) -> String {
    keep_or(value, &ids().1, "other")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seen red 2026-10-24 against the relay's old category list (chat, tasks, market, wallet,
    /// maps, game, settings, other): "a desktop report's category survives: left: \"other\" right:
    /// \"ui\"".
    #[test]
    fn a_report_keeps_a_category_and_severity_from_the_shared_list() {
        assert_eq!(category("UI"), "ui", "a desktop report's category survives");
        assert_eq!(category("Performance"), "performance");
        assert_eq!(category("chat"), "chat", "the website's page areas are on the list too");
        assert_eq!(category("Gameplay"), "other", "a word not on the list falls back");
        assert_eq!(severity("Low"), "low", "title case from the desktop is accepted");
        assert_eq!(severity("cosmetic"), "cosmetic");
        assert_eq!(severity("urgent!!"), "medium", "a severity not on the list falls back");
    }

    #[test]
    fn every_fallback_is_itself_on_the_list() {
        let (sev, cat) = ids();
        assert!(sev.iter().any(|s| s == "medium"), "medium is a severity on the list");
        assert!(cat.iter().any(|c| c == "other"), "other is a category on the list");
    }
}
