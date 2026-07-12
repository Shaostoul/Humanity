//! Scenic views (v0.825): curated named viewpoints on planet surfaces, the
//! data foundation for the camera-hub arc (task #77). Each entry pins a camera
//! to a REAL lat/lon coordinate (the same grid convention as the Blue Marble /
//! heightmap, see `terrain::planet_heightmap::latlon_to_dir`) so anyone can jump
//! to "Oahu Coast" or "Everest" and see the actual place, rendered stably in the
//! planet's rotating frame by the v0.819 frame-lock.
//!
//! Infinite-of-X: the list lives in `data/scenic_views.ron`, not in code, so new
//! places are added by editing data. The dev `camera_request.json` accepts
//! `{"view": "<name>"}`; the in-app Places list + the placeable gazebo/social-hub
//! structure (later increments) render this same data.

use serde::Deserialize;

/// One curated viewpoint. `look_offset_deg` tilts the aim from straight-down
/// (0) toward the horizon (near 90), so a coast view looks out to sea while a
/// map-like view looks down.
#[derive(Debug, Clone, Deserialize)]
pub struct ScenicView {
    pub name: String,
    #[serde(default = "default_body")]
    pub body: String,
    pub lat: f32,
    pub lon: f32,
    #[serde(default = "default_altitude_km")]
    pub altitude_km: f32,
    #[serde(default)]
    pub look_offset_deg: f32,
    #[serde(default)]
    pub description: String,
}

fn default_body() -> String {
    "earth".to_string()
}
fn default_altitude_km() -> f32 {
    2.0
}

/// The RON top-level: `(views: [ (...), (...) ])`.
#[derive(Debug, Clone, Deserialize)]
pub struct ScenicViews {
    pub views: Vec<ScenicView>,
}

impl ScenicViews {
    /// Parse the RON text. A bad file is a caller-handled error (never a panic),
    /// so a typo in the data never takes the app down.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| format!("scenic_views.ron parse error: {e}"))
    }

    /// Case-insensitive lookup by name (so "oahu coast" finds "Oahu Coast").
    pub fn find(&self, name: &str) -> Option<&ScenicView> {
        let q = name.trim().to_ascii_lowercase();
        self.views.iter().find(|v| v.name.to_ascii_lowercase() == q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"(
        views: [
            ( name: "Oahu Coast", lat: 21.3, lon: -157.8, altitude_km: 1.5, look_offset_deg: 45.0,
              description: "Hawaiian island coastline" ),
            ( name: "Everest", body: "earth", lat: 27.99, lon: 86.93, altitude_km: 8.0, look_offset_deg: 60.0 ),
            ( name: "Bare Minimum", lat: 0.0, lon: 0.0 ),
        ]
    )"#;

    #[test]
    fn parses_and_applies_defaults() {
        let s = ScenicViews::from_ron(SAMPLE).expect("parse");
        assert_eq!(s.views.len(), 3);
        let oahu = &s.views[0];
        assert_eq!(oahu.name, "Oahu Coast");
        assert!((oahu.lat - 21.3).abs() < 1e-4 && (oahu.lon + 157.8).abs() < 1e-4);
        // Defaults fill body + altitude when omitted.
        let bare = &s.views[2];
        assert_eq!(bare.body, "earth");
        assert!((bare.altitude_km - 2.0).abs() < 1e-4);
        assert_eq!(bare.look_offset_deg, 0.0);
    }

    #[test]
    fn find_is_case_insensitive() {
        let s = ScenicViews::from_ron(SAMPLE).expect("parse");
        assert!(s.find("oahu coast").is_some());
        assert!(s.find("  EVEREST ").is_some());
        assert!(s.find("nowhere").is_none());
    }

    #[test]
    fn bad_ron_is_an_error_not_a_panic() {
        assert!(ScenicViews::from_ron("(views: [ not valid ]").is_err());
    }

    #[test]
    fn shipped_scenic_views_file_parses() {
        // The committed data must always load (a syntax error would silently
        // disable every Place). CARGO_MANIFEST_DIR points at the repo root.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/scenic_views.ron");
        let text = std::fs::read_to_string(path).expect("scenic_views.ron present");
        let s = ScenicViews::from_ron(&text).expect("shipped scenic_views.ron parses");
        assert!(!s.views.is_empty(), "at least one curated view");
        // Every view names a body and sane coordinates.
        for v in &s.views {
            assert!(!v.name.is_empty(), "view has a name");
            assert!(v.lat >= -90.0 && v.lat <= 90.0, "{}: lat range", v.name);
            assert!(v.lon >= -180.0 && v.lon <= 180.0, "{}: lon range", v.name);
        }
    }
}
