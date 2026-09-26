//! Quality grades for crafted goods (2026-09-26; crafting depth, the last
//! item of the gameplay arc's step 5, docs/design/gameplay-gaps-2026-09-25.md).
//!
//! The grades were already designed and sitting unused in
//! `data/manufacturing.ron` (`quality_levels`: defective, poor, standard,
//! good, excellent, masterwork, each with a score band, a price multiplier
//! and a durability multiplier). This reads them and grades what a person
//! makes by hand:
//!
//! - Only DURABLE goods are graded (items with an items.csv durability:
//!   tools, weapons, clothing, furniture). Materials and food stay ungraded,
//!   so a stack of planks never splits six ways by grade.
//! - The grade comes from the crafter's level in the recipe's skill, with
//!   the file's own formula: score = skill factor plus a random -0.1..0.1,
//!   clamped to 0..1, then the band it falls in. There is no assembly-line
//!   error rate for a person at a bench.
//! - A grade changes how long a tool lasts (durability x multiplier; a
//!   defective one breaks on first use) and what a vendor pays for it
//!   (price x multiplier; defective goods are not sellable).
//! - Automated machines turn out standard goods.
//!
//! `ItemStack::quality` stores the grade as index + 1 into the levels,
//! worst first; 0 means ungraded (materials, bought goods, older saves).

use serde::Deserialize;

/// One grade, as `data/manufacturing.ron` writes it.
#[derive(Debug, Clone, Deserialize)]
pub struct QualityLevel {
    pub id: String,
    pub name: String,
    pub threshold_min: f32,
    pub threshold_max: f32,
    #[serde(default = "one")]
    pub price_multiplier: f32,
    #[serde(default = "one")]
    pub durability_multiplier: f32,
    #[serde(default = "yes")]
    pub sellable: bool,
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

/// The grades, worst first.
#[derive(Debug, Clone, Default)]
pub struct QualityLevels {
    pub levels: Vec<QualityLevel>,
}

/// A beginner's skill factor: level 1 of any skill is a competent beginner
/// whose work is standard most of the time (score 0.35 to 0.55), and the
/// skill's top level reaches masterwork now and then. A design choice, not
/// a measured number; the file's formula leaves the factor to the game.
pub const BEGINNER_FACTOR: f32 = 0.45;

impl QualityLevels {
    pub const FILE: &'static str = "manufacturing.ron";

    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            quality_levels: Vec<QualityLevel>,
        }
        let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let f: File = ron::from_str(s).map_err(|e| format!("{}: {e}", Self::FILE))?;
        let mut levels = f.quality_levels;
        levels.sort_by(|a, b| a.threshold_min.partial_cmp(&b.threshold_min).unwrap_or(std::cmp::Ordering::Equal));
        Ok(Self { levels })
    }

    /// The grade stored as `q` (index + 1); None for 0 (ungraded).
    pub fn get(&self, q: u8) -> Option<&QualityLevel> {
        (q as usize).checked_sub(1).and_then(|i| self.levels.get(i))
    }

    pub fn name(&self, q: u8) -> Option<&str> {
        self.get(q).map(|l| l.name.as_str())
    }

    /// How long a tool of grade `q` lasts, from its base durability. At least
    /// one use: a defective tool (multiplier 0) breaks the first time.
    pub fn durability(&self, base: u32, q: u8) -> u32 {
        if base == 0 {
            return 0;
        }
        let m = self.get(q).map_or(1.0, |l| l.durability_multiplier);
        ((base as f32 * m).round() as u32).max(1)
    }

    /// What grade `q` does to a vendor's price: its multiplier, or 0 for a
    /// grade that is not sellable. Ungraded goods sell at the base price.
    pub fn price_multiplier(&self, q: u8) -> f32 {
        self.get(q).map_or(1.0, |l| if l.sellable { l.price_multiplier } else { 0.0 })
    }

    /// The grade a 0..1 score falls in (its band; the top band includes 1.0).
    /// 0 when no grades are loaded.
    pub fn grade_for(&self, score: f32) -> u8 {
        let last = self.levels.len();
        for (i, l) in self.levels.iter().enumerate() {
            if score >= l.threshold_min && (score < l.threshold_max || i + 1 == last) {
                return (i + 1) as u8;
            }
        }
        if last > 0 && score < self.levels[0].threshold_min {
            return 1;
        }
        0
    }

    /// The grade an automated machine turns out: standard.
    pub fn standard(&self) -> u8 {
        self.levels.iter().position(|l| l.id == "standard").map_or(0, |i| (i + 1) as u8)
    }
}

/// The crafter's skill as a 0..1 factor: level 1 (or untrained) is
/// `BEGINNER_FACTOR`, the skill's max level is 1.0, linear between.
pub fn skill_factor(level: u32, max_level: u32) -> f32 {
    let level = level.max(1);
    if max_level <= 1 {
        return BEGINNER_FACTOR;
    }
    let t = ((level - 1) as f32 / (max_level - 1) as f32).clamp(0.0, 1.0);
    BEGINNER_FACTOR + (1.0 - BEGINNER_FACTOR) * t
}

/// manufacturing.ron's score: the skill factor plus a random -0.1..0.1 (from
/// a uniform `roll` in 0..1), clamped to 0..1.
pub fn score(skill_factor: f32, roll: f32) -> f32 {
    (skill_factor + roll * 0.2 - 0.1).clamp(0.0, 1.0)
}

/// Read the grades from the data folder (or the copy built into the exe).
/// Missing or broken: no grades, and everything stays ungraded.
pub fn load(data_dir: &std::path::Path) -> QualityLevels {
    match crate::embedded_data::read_data_or_embedded(data_dir, QualityLevels::FILE).map(|s| QualityLevels::from_ron(s.as_bytes())) {
        Some(Ok(q)) => q,
        Some(Err(e)) => {
            log::warn!("{e}; crafted goods are ungraded this session");
            QualityLevels::default()
        }
        None => QualityLevels::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> QualityLevels {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/manufacturing.ron")).unwrap();
        QualityLevels::from_ron(&bytes).expect("manufacturing.ron quality_levels")
    }

    #[test]
    fn the_grades_load_worst_first_and_bands_cover_zero_to_one() {
        let q = levels();
        let ids: Vec<&str> = q.levels.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["defective", "poor", "standard", "good", "excellent", "masterwork"]);
        assert_eq!(q.name(q.grade_for(0.0)), Some("Defective"));
        assert_eq!(q.name(q.grade_for(0.5)), Some("Standard"));
        assert_eq!(q.name(q.grade_for(1.0)), Some("Masterwork"));
        assert_eq!(q.grade_for(0.2), q.grade_for(0.3), "band edges belong to the upper band");
        assert_eq!(q.standard(), 3);
        assert!(q.name(0).is_none(), "0 is ungraded");
    }

    #[test]
    fn a_grade_changes_how_long_a_tool_lasts_and_what_it_sells_for() {
        let q = levels();
        assert_eq!(q.durability(200, 0), 200, "ungraded: the base");
        assert_eq!(q.durability(200, q.standard()), 200);
        assert_eq!(q.durability(200, 1), 1, "defective breaks on first use");
        assert!(q.durability(200, 6) > 400, "masterwork lasts over twice as long");
        assert_eq!(q.durability(0, 6), 0, "no durability stays none");
        assert_eq!(q.price_multiplier(1), 0.0, "defective is not sellable");
        assert_eq!(q.price_multiplier(0), 1.0);
        assert!(q.price_multiplier(4) > 1.0);
    }

    #[test]
    fn skill_moves_the_grade() {
        let q = levels();
        let beginner = skill_factor(1, 20);
        let master = skill_factor(20, 20);
        assert_eq!(skill_factor(0, 20), beginner, "untrained crafts like a beginner");
        // Across the whole roll range a beginner makes poor to standard work
        // and a master good work or better.
        for roll in [0.0_f32, 0.25, 0.5, 0.75, 0.999] {
            let b = q.grade_for(score(beginner, roll));
            let m = q.grade_for(score(master, roll));
            assert!((2..=3).contains(&b), "beginner roll {roll}: grade {b}");
            assert!(m >= 5, "master roll {roll}: grade {m}");
        }
    }
}
