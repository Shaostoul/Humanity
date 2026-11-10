//! The whole-Earth map's data files (rung 2, 2026-11-09): `data/maps/levels.ron` (the data levels,
//! the lattice, the road classes and the shore band's numbers) and `data/maps/places.ron` (named
//! places: the Maps page's views, the builder's home set and its geography gates). Plain serde
//! structs; the builder (`map_build`) and the run time (`map_ground`) read the same rows.
//! Relay-safe.

use serde::Deserialize;
use std::path::Path;

/// One beach profile: the A of `h = datum - A * x^(2/3)` for shores whose land cover is one of
/// `cover` (ESA WorldCover classes).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ShoreProfile {
    pub name: String,
    pub cover: Vec<u8>,
    pub a: f64,
}

/// The shore band's numbers (docs/design/whole-earth-maps.md 5.2; every field's why is in the file).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ShoreParams {
    pub datum_m: f64,
    pub lake_datum_m: f64,
    pub wet_offshore_m: f64,
    pub wet_depth_m: f64,
    pub floor_cells: f64,
    pub floor_release_grade: f64,
    pub min_band_m: f64,
    pub band_spacings: f64,
    pub cap_slope: f64,
    pub sea_blend_m: f64,
    pub reach_m: f64,
    pub profiles: Vec<ShoreProfile>,
    pub default_a: f64,
}

impl ShoreParams {
    /// The beach profile's A for a shore whose land cover is `cover`.
    pub fn profile_a(&self, cover: u8) -> f64 {
        self.profiles.iter().find(|p| p.cover.contains(&cover)).map_or(self.default_a, |p| p.a)
    }
}

/// One road class's bed: the steepest grade and the smoothing window along the road.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BedClass {
    pub class: u8,
    pub grade: f64,
    pub smooth_m: f64,
}

/// Road beds and building pads (docs/design/whole-earth-maps.md 6.1 to 6.3; every field's why is
/// in the file).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BedParams {
    pub shoulder_m: f64,
    pub side_slope: f64,
    pub wall_after_m: f64,
    pub wall_slope: f64,
    pub reach_m: f64,
    pub profile_step_m: f64,
    pub pin_blend_m: f64,
    pub pin_grade: f64,
    pub classes: Vec<BedClass>,
    pub apron_m: f64,
    pub pad_sample_m: f64,
    pub geometry_spacings: f64,
    pub first_depth: u8,
}

impl BedParams {
    /// A class's row; the last row (the most permissive) for a class the file does not list.
    pub fn class(&self, class: u8) -> Option<&BedClass> {
        self.classes.iter().find(|c| c.class == class).or(self.classes.last())
    }

    /// How far from its bed or pad the cut or fill may stand from the natural ground at `x`
    /// metres out: the side slope, then the wall past `wall_after_m`.
    pub fn slope_room(&self, x: f64) -> f64 {
        if x <= self.wall_after_m {
            self.side_slope * x
        } else {
            self.side_slope * self.wall_after_m + self.wall_slope * (x - self.wall_after_m)
        }
    }
}

/// How far an installed pack's terrain refines past the planet's own cap, and how near the camera.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PackDetail {
    pub max_depth: u8,
    pub within_m: f64,
}

/// `data/maps/levels.ron`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapLevels {
    pub levels: Vec<u8>,
    pub local_pack_depth: u8,
    pub lattice_segments: u32,
    pub sea_levels: Vec<u8>,
    pub road_classes: Vec<(String, u8)>,
    pub shore: ShoreParams,
    pub detail_first_depth: u8,
    pub detail_shore_mask_m: f64,
    pub edge_blend_m: f64,
    pub pack_detail: PackDetail,
    pub beds: BedParams,
}

impl MapLevels {
    /// Parse the file's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let l: MapLevels = ron::from_str(text).map_err(|e| format!("levels.ron: {e}"))?;
        if l.levels.is_empty() || l.levels.windows(2).any(|w| w[0] >= w[1]) {
            return Err("levels.ron: levels must be ascending and not empty".into());
        }
        if !l.lattice_segments.is_power_of_two() || l.lattice_segments > 1 << 10 {
            return Err("levels.ron: lattice_segments must be a power of two".into());
        }
        if l.levels[0] != l.local_pack_depth || l.sea_levels.iter().any(|s| !l.levels.contains(s)) {
            return Err("levels.ron: the pack depth is the first level and the sea levels are levels".into());
        }
        Ok(l)
    }

    /// Read `<data>/maps/levels.ron`.
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let p = data_dir.join("maps").join("levels.ron");
        Self::parse(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
    }

    /// The finest level (the packs' vector level).
    pub fn finest(&self) -> u8 {
        *self.levels.last().unwrap_or(&12)
    }

    /// The road class of an OSM `highway` value.
    pub fn road_class(&self, highway: &str) -> Option<u8> {
        self.road_classes.iter().find(|(k, _)| k == highway).map(|(_, c)| *c)
    }
}

/// A place's geography gates (what the builder checks a pack holding it against).
#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
pub struct PlaceGates {
    pub min_roads: usize,
    pub min_buildings: usize,
    pub road_named_any: Vec<String>,
    pub water_named_any: Vec<String>,
    pub sea_at: Vec<(f64, f64)>,
    pub land_at: Vec<(f64, f64)>,
    pub lake_at: Vec<(f64, f64)>,
}

/// One named place.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Place {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub half_east_m: f64,
    pub half_north_m: f64,
    pub home: bool,
    pub gates: PlaceGates,
}

/// `data/maps/places.ron`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapPlaces {
    pub places: Vec<Place>,
}

impl MapPlaces {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| format!("places.ron: {e}"))
    }

    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let p = data_dir.join("maps").join("places.ron");
        Self::parse(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> std::path::PathBuf {
        Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
    }

    /// The shipped files parse and say what the plan says (levels 6, 9, 12; the 64-segment
    /// lattice; Silverdale and Seattle Center at home). Seen red 2026-11-09 with levels.ron's
    /// levels written [6, 12, 9]: "levels.ron: levels must be ascending and not empty".
    #[test]
    fn the_shipped_map_files_parse() {
        let l = MapLevels::load(&data()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((l.levels.as_slice(), l.local_pack_depth, l.lattice_segments), (&[6u8, 9, 12][..], 6, 64));
        assert_eq!(l.road_class("residential"), Some(3));
        assert_eq!(l.road_class("motorway_link"), Some(0));
        assert_eq!(l.road_class("proposed"), None);
        assert_eq!(l.shore.profile_a(60), 0.10);
        assert_eq!(l.shore.profile_a(10), l.shore.default_a, "a tree-covered shore takes the default");
        assert_eq!((l.pack_detail.max_depth, l.beds.first_depth), (20, 15));
        assert_eq!(l.beds.class(3).map(|c| c.grade), Some(0.15), "residential streets keep to the plan's 15%");
        assert_eq!(l.beds.slope_room(10.0), 5.0, "1 in 2 back to the natural ground");
        let p = MapPlaces::load(&data()).unwrap_or_else(|e| panic!("{e}"));
        let home: Vec<&str> = p.places.iter().filter(|p| p.home).map(|p| p.name.as_str()).collect();
        assert_eq!(home, vec!["Silverdale", "Seattle Center"]);
        assert!(MapLevels::parse(&std::fs::read_to_string(data().join("maps/levels.ron")).unwrap().replace("levels: [6, 9, 12]", "levels: [6, 12, 9]")).is_err());
    }
}
