//! Character-select showroom data (v0.441): the backdrops you can preview your avatar
//! against. Pure data (serde); the showroom render + camera live in `lib.rs` (native) and
//! the panel in `gui::pages::showroom`.

use serde::Deserialize;
use std::path::Path;

/// One showroom backdrop: a ground tint + ambient level the avatar is previewed against.
#[derive(Debug, Clone, Deserialize)]
pub struct Backdrop {
    pub id: String,
    pub name: String,
    /// Linear RGB of the ground disc under the avatar.
    pub ground: (f32, f32, f32),
    /// Ambient light level 0..1 (brightens the avatar).
    pub ambient: f32,
}

/// Load the backdrop list from `data/showroom/backdrops.ron`. Returns a single neutral
/// fallback if the file is missing/invalid so the showroom always has at least one.
pub fn load_backdrops(data_dir: &Path) -> Vec<Backdrop> {
    let path = data_dir.join("showroom").join("backdrops.ron");
    match std::fs::read_to_string(&path) {
        Ok(text) => match ron::from_str::<Vec<Backdrop>>(&text) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => fallback(),
            Err(e) => {
                log::warn!("showroom: failed to parse backdrops.ron: {e}");
                fallback()
            }
        },
        Err(_) => fallback(),
    }
}

fn fallback() -> Vec<Backdrop> {
    vec![Backdrop { id: "space".into(), name: "Floating in space".into(), ground: (0.05, 0.05, 0.08), ambient: 0.3 }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shipped_backdrops() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let b = load_backdrops(&dir);
        assert!(b.len() >= 5, "shipped backdrops should parse, got {}", b.len());
        assert!(b.iter().any(|x| x.id == "mars"), "mars backdrop present");
    }
}
