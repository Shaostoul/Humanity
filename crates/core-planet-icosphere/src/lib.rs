use serde::{Deserialize, Serialize};

pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

pub fn triangle_count(subdivision: u32) -> u64 {
    20u64.saturating_mul(4u64.saturating_pow(subdivision))
}

pub fn average_edge_length_m(radius_m: f64, subdivision: u32) -> f64 {
    let tri = triangle_count(subdivision) as f64;
    let area_sphere = 4.0 * std::f64::consts::PI * radius_m * radius_m;
    let area_tri = area_sphere / tri;
    // Equilateral triangle area = sqrt(3)/4 * a^2 => a = sqrt(4A/sqrt(3))
    ((4.0 * area_tri) / 3.0_f64.sqrt()).sqrt()
}

pub fn subdivision_for_target_edge(radius_m: f64, target_edge_m: f64, max_subdivision: u32) -> u32 {
    let mut s = 0;
    while s < max_subdivision {
        if average_edge_length_m(radius_m, s) <= target_edge_m {
            return s;
        }
        s += 1;
    }
    max_subdivision
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LodBand {
    pub max_distance_m: u32,
    pub subdivision: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LodProfile {
    pub bands: Vec<LodBand>,
}

impl LodProfile {
    pub fn earth_default() -> Self {
        Self {
            bands: vec![
                LodBand { max_distance_m: 400, subdivision: 12 },
                LodBand { max_distance_m: 1_000, subdivision: 10 },
                LodBand { max_distance_m: 4_000, subdivision: 8 },
                LodBand { max_distance_m: 15_000, subdivision: 6 },
                LodBand { max_distance_m: 60_000, subdivision: 4 },
                LodBand { max_distance_m: u32::MAX, subdivision: 2 },
            ],
        }
    }

    pub fn subdivision_for_distance(&self, distance_m: u32) -> u8 {
        self.bands
            .iter()
            .find(|b| distance_m <= b.max_distance_m)
            .map(|b| b.subdivision)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryHint {
    Keep,
    Split,
    Merge,
}

pub fn lod_transition_hint(current_subdivision: u8, desired_subdivision: u8) -> RecoveryHint {
    if desired_subdivision > current_subdivision {
        RecoveryHint::Split
    } else if desired_subdivision < current_subdivision {
        RecoveryHint::Merge
    } else {
        RecoveryHint::Keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_count_growth_is_correct() {
        assert_eq!(triangle_count(0), 20);
        assert_eq!(triangle_count(1), 80);
        assert_eq!(triangle_count(2), 320);
        assert_eq!(triangle_count(3), 1280);
    }

    #[test]
    fn edge_length_shrinks_with_subdivision() {
        let e0 = average_edge_length_m(EARTH_RADIUS_M, 0);
        let e4 = average_edge_length_m(EARTH_RADIUS_M, 4);
        let e8 = average_edge_length_m(EARTH_RADIUS_M, 8);
        assert!(e4 < e0);
        assert!(e8 < e4);
    }

    #[test]
    fn target_edge_resolution_estimate() {
        let s = subdivision_for_target_edge(EARTH_RADIUS_M, 1.0, 30);
        // Rough expected range for Earth-scale 1m average edges is very high.
        assert!(s >= 20);
    }

    #[test]
    fn lod_profile_selects_near_higher_detail() {
        let p = LodProfile::earth_default();
        assert!(p.subdivision_for_distance(200) > p.subdivision_for_distance(20_000));
    }
}
