//! A SERVER'S GAMEPLAY RULES, THE GAME'S SIDE (2026-11-21; docs/design/server-rules.md). The
//! operator, that day: "Let's make sure the first server we have setup, via united-humanity is the
//! forgiving game mode. I think realistic will be too much for most people."
//!
//! In solo play each gameplay level (data/gameplay/levels.json) is the person's own choice,
//! `SettingsState::gameplay`. In a server's shared world it is the SERVER'S ruleset
//! (src/relay/storage/gameplay_rules.rs), held in `SettingsState::server_rules` from the welcome
//! until the game leaves that world (engine/server_rules.rs), so everyone there plays by the
//! same rules. Every system reads a level through here ([`SettingsState::level`], the map
//! [`SettingsState::rules_map`] for the readers that take one, and
//! [`SettingsState::vitals_drain_rate`] for the drain's number), so none of them can read the
//! person's own levels while joined. A setting the server's levels do not name plays at its
//! default (the level a new player gets); a setting marked `personal` (the bodies' animation)
//! stays the person's own everywhere. The person's own choices are never touched: leaving the
//! world gives them back as they were.

use std::collections::BTreeMap;

use super::gameplay_levels;
use super::SettingsState;

impl SettingsState {
    /// The level in play for the gameplay setting `id` (data/gameplay/levels.json): the server's
    /// in its shared world, else the person's own; the setting's default where the one in play
    /// names none.
    pub fn level(&self, id: &str) -> &str {
        gameplay_levels::level_of(self.levels_for(id), id)
    }

    /// The number of the level in play for `id` (hostile wildlife's share of a pack), 0 when the
    /// setting or its number is missing.
    pub fn level_value(&self, id: &str) -> f32 {
        gameplay_levels::value_of(self.levels_for(id), id)
    }

    /// Choose `level` for the gameplay setting `id`: the person's OWN choice (a rig's verb, a
    /// test, Settings itself), which plays whenever no server rules.
    pub fn set_level(&mut self, id: &str, level: &str) {
        self.gameplay.insert(id.to_string(), level.to_string());
    }

    /// The map of levels in play for the settings a server rules: its levels in its shared world,
    /// else the person's own (for a reader that takes the whole map, as the garden's do).
    pub fn rules_map(&self) -> &BTreeMap<String, String> {
        self.server_rules.as_ref().map_or(&self.gameplay, |r| &r.levels)
    }

    /// The vitals drain in play: the server's level's number in its shared world, else the
    /// person's own slider (whose levels are points on it).
    pub fn vitals_drain_rate(&self) -> f32 {
        match &self.server_rules {
            Some(r) => gameplay_levels::value_of(&r.levels, "vitals_drain"),
            None => self.vitals_drain,
        }
    }

    /// Is `id` the server's in play now (in its shared world, and not a personal setting)?
    pub fn server_rules_setting(&self, id: &str) -> bool {
        self.server_rules.is_some() && !gameplay_levels::setting(id).is_some_and(|s| s.personal)
    }

    /// The map `id` is read from: the server's when it rules that setting, else the person's own.
    fn levels_for(&self, id: &str) -> &BTreeMap<String, String> {
        match &self.server_rules {
            Some(r) if self.server_rules_setting(id) => &r.levels,
            _ => &self.gameplay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::storage::GameplayRules;

    /// EVERY PRESET NAMES REAL LEVELS, AND REALISTIC NAMES EVERY SETTING A SERVER RULES
    /// (2026-11-21): each level a preset of data/gameplay/presets.json names is one its setting
    /// lists in data/gameplay/levels.json, no preset names a personal setting, and Realistic names
    /// every other setting, so a setting added to levels.json cannot play softened under Realistic
    /// without anyone deciding it.
    ///
    /// Seen red 2026-11-21 with cooking left out of Realistic: "Realistic names no level for
    /// cooking".
    #[test]
    fn every_ruled_setting_has_a_realistic_level() {
        let mut wrong = Vec::new();
        for p in &crate::relay::storage::gameplay_rules::presets().presets {
            for (id, level) in &p.levels {
                match gameplay_levels::setting(id) {
                    None => wrong.push(format!("{}: names {id}, which levels.json does not have", p.id)),
                    Some(s) if s.personal => wrong.push(format!("{}: names {id}, a personal setting", p.id)),
                    Some(s) if s.level(level).is_none() => wrong.push(format!("{}: {id} has no level {level}", p.id)),
                    Some(_) => {}
                }
            }
        }
        let realistic = crate::relay::storage::gameplay_rules::preset("realistic").expect("Realistic is a preset");
        for s in gameplay_levels::file().settings.iter().filter(|s| !s.personal) {
            if !realistic.levels.contains_key(&s.id) {
                wrong.push(format!("Realistic names no level for {}", s.id));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// The person's own levels, all at their gentlest that differ from Realistic.
    fn own_off() -> SettingsState {
        let mut s = SettingsState::default();
        for id in ["body_heat", "illness", "animal_care", "carry", "death", "mining_drone", "skill_learning", "picking"] {
            s.set_level(id, "off");
        }
        s.set_level("animation", "simplified");
        s.vitals_drain = 0.0;
        s
    }

    /// A JOINED GAME READS THE SERVER'S LEVELS, NOT THE PERSON'S, AND LEAVING GIVES THEM BACK
    /// (2026-11-21). With the person's own levels at Off and the server's Realistic, every setting
    /// the server rules reads Realistic's level (the vitals drain its number), a personal one (the
    /// bodies' animation) stays the person's, and the person's own map is untouched; under a
    /// Forgiving server every setting reads its default; and with the rules gone (the game left
    /// the world) every level is the person's own again.
    ///
    /// Seen red 2026-11-21 with `level` reading `self.gameplay` as before: "animal_care reads the
    /// server's realistic while joined / left: \"off\" / right: \"realistic\"".
    #[test]
    fn a_joined_game_reads_the_servers_levels_and_leaving_gives_the_persons_back() {
        let mut s = own_off();
        let realistic = GameplayRules::preset("realistic").expect("Realistic is a preset");
        s.server_rules = Some(GameplayRules { levels: realistic.levels_in_play(), ..realistic });
        for (id, want) in &s.server_rules.clone().unwrap().levels {
            assert_eq!(s.level(id), want.as_str(), "{id} reads the server's {want} while joined");
        }
        assert_eq!(s.vitals_drain_rate(), 1.0, "the drain plays the server's real time, not the person's 0");
        assert_eq!(s.level_value("hostile_wildlife"), 1.0, "a server's Normal wildlife");
        assert_eq!(s.level("animation"), "simplified", "how the bodies are drawn stays the person's");
        assert_eq!(s.gameplay.get("body_heat").map(String::as_str), Some("off"), "the person's own choice is untouched");
        s.server_rules = GameplayRules::preset("forgiving");
        for setting in &gameplay_levels::file().settings {
            if !setting.personal {
                assert_eq!(s.level(&setting.id), setting.default, "a Forgiving server plays {} at its default", setting.id);
            }
        }
        s.server_rules = None;
        assert_eq!(s.level("body_heat"), "off", "left the world: the person's own again");
        assert_eq!(s.vitals_drain_rate(), 0.0, "and their own drain");
    }
}
