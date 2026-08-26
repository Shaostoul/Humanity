//! Third-party attributions, loaded from `data/credits.ron`.
//!
//! Several of these are licence OBLIGATIONS, not courtesies. OpenStreetMap's
//! ODbL is the sharp one: a rendered view of OSM data is a Produced Work and
//! needs a visible notice wherever it is shown, which a credit buried in the
//! repository does not satisfy. That is why [`osm_notice`] exists as a single
//! shared string rather than a literal repeated at each surface - two copies
//! drift, and a drifted attribution is a broken one.
//!
//! Infinite-of-X: sources are data rows. The list only ever grows.

use std::path::Path;

/// One third-party source.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CreditSource {
    pub id: String,
    pub name: String,
    /// Plain-language description of what it is used for, written for a player
    /// rather than for a lawyer.
    pub used_for: String,
    pub licence: String,
    /// The exact text that must be shown. Copied from the licence where the
    /// licence specifies wording.
    pub notice: String,
    pub url: String,
    /// True when the licence REQUIRES the notice be shown to users, as opposed
    /// to it being a courtesy.
    #[serde(default)]
    pub attribution_required: bool,
    /// Surfaces where the notice actually appears. Recorded so an audit can
    /// check the obligation is met rather than merely documented.
    #[serde(default)]
    pub shown_in: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Credits {
    pub sources: Vec<CreditSource>,
}

impl Credits {
    /// Load `data/credits.ron`. A missing or broken file yields an empty list
    /// and a warning rather than a panic: a bad data edit must not make the
    /// game unstartable.
    pub fn load(data_dir: &Path) -> Self {
        let p = data_dir.join("credits.ron");
        match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| ron::from_str::<Credits>(&t).map_err(|e| e.to_string()))
        {
            Ok(c) => c,
            Err(e) => {
                log::warn!("credits: {} failed to load: {e}", p.display());
                Credits::default()
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&CreditSource> {
        self.sources.iter().find(|s| s.id == id)
    }
}

/// The OpenStreetMap notice, for the two surfaces that DRAW OSM data: the Maps
/// planet view and the in-world region geometry.
///
/// A constant rather than a data lookup because it is on the render path and
/// must never be absent - if `credits.ron` were missing or malformed, the
/// obligation would still stand. The data file carries the same string for the
/// Credits page; [`osm_notice_matches_data`] holds the two together.
pub const OSM_NOTICE: &str = "Map data (c) OpenStreetMap contributors (ODbL)";

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    #[test]
    fn shipped_credits_file_parses() {
        let c = Credits::load(&data_dir());
        assert!(
            !c.sources.is_empty(),
            "data/credits.ron must ship and parse; an empty list means it silently failed to load"
        );
    }

    /// The render-path constant and the data file must agree, or the Credits
    /// page and the map footer would show different attributions for the same
    /// obligation.
    #[test]
    fn osm_notice_matches_data() {
        let c = Credits::load(&data_dir());
        let osm = c
            .get("openstreetmap")
            .expect("credits.ron must carry the openstreetmap row");
        assert_eq!(
            osm.notice, OSM_NOTICE,
            "the OSM notice in data/credits.ron and credits::OSM_NOTICE have drifted"
        );
    }

    /// Every source whose licence REQUIRES attribution must name at least one
    /// surface where the notice actually appears. This is the check that stops
    /// an obligation from being quietly recorded and never honoured.
    #[test]
    fn every_required_attribution_names_a_surface() {
        let c = Credits::load(&data_dir());
        for s in c.sources.iter().filter(|s| s.attribution_required) {
            assert!(
                !s.shown_in.is_empty(),
                "{} requires attribution but names no surface that shows it",
                s.name
            );
            assert!(
                !s.notice.trim().is_empty(),
                "{} requires attribution but has no notice text",
                s.name
            );
        }
    }

    /// OSM is the one with two distinct obligations, and the rendered-view half
    /// is the one that is easy to forget. Both drawing surfaces must be listed.
    #[test]
    fn osm_names_both_drawing_surfaces() {
        let c = Credits::load(&data_dir());
        let osm = c.get("openstreetmap").expect("openstreetmap row");
        let joined = osm.shown_in.join(" | ").to_lowercase();
        assert!(
            joined.contains("maps"),
            "OSM must credit the Maps planet view, got {:?}",
            osm.shown_in
        );
        assert!(
            joined.contains("in-world"),
            "OSM must credit the in-world region view - a rendered Produced Work \
             needs a visible notice, got {:?}",
            osm.shown_in
        );
    }
}
