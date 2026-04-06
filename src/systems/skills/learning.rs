//! Learning-by-doing — skills improve through practice, not XP spending.
//!
//! Learning curves loaded from `data/learning_curves.csv`.

use serde::{Deserialize, Serialize};

/// A skill with level and accumulated practice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub level: u32,
    pub practice_hours: f32,
}

impl Skill {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            level: 0,
            practice_hours: 0.0,
        }
    }

    /// Add practice time and check for level-up (stub — needs learning curve data).
    pub fn add_practice(&mut self, hours: f32) {
        self.practice_hours += hours;
        // TODO: check against learning curve CSV for level threshold
    }
}
