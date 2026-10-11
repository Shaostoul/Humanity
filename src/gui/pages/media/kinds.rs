//! The kinds of item on the Media page (pictures, videos, models, files), read from
//! `data/media/kinds.json` so the list is data, not code (CLAUDE.md "Infinite-of-X"). The relay
//! decides an item's kind from its file's type and filters by the same words; this file only
//! gives each one its name and the mark a tile without a thumbnail shows.

use serde::Deserialize;

/// One kind: the relay's word for it, the filter's name, the name of one item, and its mark.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Kind {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub one: String,
    #[serde(default)]
    pub mark: String,
}

#[derive(Deserialize)]
struct KindsFile {
    kinds: Vec<Kind>,
}

/// The data file, relative to the data folder.
pub const KINDS_FILE: &str = "media/kinds.json";

/// The copy built into the exe, used when the data folder's copy cannot be read.
const BUILT_IN: &str = include_str!("../../../../data/media/kinds.json");

/// Read a kinds file: refused when it is not one, lists none, or has an empty id or name.
pub fn parse_kinds(text: &str) -> Result<Vec<Kind>, String> {
    let file: KindsFile = serde_json::from_str(text).map_err(|e| format!("not a kinds file: {e}"))?;
    if file.kinds.is_empty() {
        return Err("it lists no kinds".into());
    }
    if let Some(k) = file.kinds.iter().find(|k| k.id.trim().is_empty() || k.label.trim().is_empty()) {
        return Err(format!("the kind {:?} has no id or no name", k.id));
    }
    Ok(file.kinds)
}

/// The kinds, read once: the data folder's copy, else the built-in one (logged), else none.
pub fn kinds() -> &'static [Kind] {
    static KINDS: std::sync::OnceLock<Vec<Kind>> = std::sync::OnceLock::new();
    KINDS.get_or_init(|| {
        let path = crate::data_dir().join(KINDS_FILE);
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse_kinds(&t)) {
            Ok(k) => k,
            Err(why) => {
                log::warn!("Media: {} could not be used ({why}); the copy built into the app is used", path.display());
                parse_kinds(BUILT_IN).unwrap_or_default()
            }
        }
    })
}

/// The kind named `id`, if the file lists it.
pub fn kind(id: &str) -> Option<&'static Kind> {
    kinds().iter().find(|k| k.id == id)
}

/// What one item of kind `id` is called on a tile: the file's `one`, else its name, else the
/// word itself (a kind the relay knows and this file does not still reads as something).
pub fn one_name(id: &str) -> String {
    match kind(id) {
        Some(k) if !k.one.is_empty() => k.one.clone(),
        Some(k) => k.label.clone(),
        None => id.to_string(),
    }
}

/// The mark a tile of kind `id` paints when it has no thumbnail.
pub fn mark(id: &str) -> &'static str {
    kind(id).map(|k| k.mark.as_str()).unwrap_or("page")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE FOUR KINDS ARE THE RELAY'S: the built-in file lists exactly the words the relay
    /// stores and filters by (storage/media.rs `KINDS`), so a filter never sends a kind the
    /// relay refuses.
    ///
    /// Seen red 2026-10-11 with "model" spelled "models" in the file: "left: [\"picture\",
    /// \"video\", \"models\", \"file\"] / right: [\"picture\", \"video\", \"model\", \"file\"]".
    #[test]
    fn the_kinds_are_the_relays_kinds() {
        let ours: Vec<String> = parse_kinds(BUILT_IN).expect("the built-in kinds parse").into_iter().map(|k| k.id).collect();
        let relay: Vec<String> = crate::relay::storage::media::KINDS.iter().map(|k| k.to_string()).collect();
        assert_eq!(ours, relay);
        assert!(parse_kinds(r#"{"kinds": []}"#).is_err(), "an empty list is refused");
        assert_eq!(one_name("model"), "3D model");
        assert_eq!(one_name("something_new"), "something_new", "an unknown kind still reads");
    }
}
