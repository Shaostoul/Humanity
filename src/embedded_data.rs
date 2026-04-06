//! Compile-time embedded data files for offline operation.
//!
//! All critical game data is embedded via `include_str!` so the app works
//! without any external files. The AssetManager tries disk first (for modding)
//! and falls back to these constants when the file is missing.

// ── Core game data (CSV) ────────────────────────────────────────────
pub const ITEMS_CSV: &str = include_str!("../data/items.csv");
pub const RECIPES_CSV: &str = include_str!("../data/recipes.csv");
pub const MATERIALS_CSV: &str = include_str!("../data/materials.csv");
pub const COMPONENTS_CSV: &str = include_str!("../data/components.csv");
pub const PLANTS_CSV: &str = include_str!("../data/plants.csv");
pub const GAME_CSV: &str = include_str!("../data/game.csv");

// ── Skills ──────────────────────────────────────────────────────────
pub const SKILLS_CSV: &str = include_str!("../data/skills/skills.csv");

// ── Chemistry ───────────────────────────────────────────────────────
pub const ELEMENTS_CSV: &str = include_str!("../data/chemistry/elements.csv");
pub const ALLOYS_CSV: &str = include_str!("../data/chemistry/alloys.csv");
pub const COMPOUNDS_CSV: &str = include_str!("../data/chemistry/compounds.csv");
pub const GASES_CSV: &str = include_str!("../data/chemistry/gases.csv");
pub const TOXINS_CSV: &str = include_str!("../data/chemistry/toxins.csv");

// ── Asteroids ───────────────────────────────────────────────────────
pub const ASTEROID_TYPES_CSV: &str = include_str!("../data/asteroids/types.csv");

// ── JSON data ───────────────────────────────────────────────────────
pub const GLOSSARY_JSON: &str = include_str!("../data/glossary.json");
pub const SOLAR_SYSTEM_JSON: &str = include_str!("../data/solar_system/bodies.json");
pub const SOLAR_SYSTEM_LEGACY_JSON: &str = include_str!("../data/solar-system.json");
pub const TOOLS_CATALOG_JSON: &str = include_str!("../data/tools/catalog.json");
pub const CITIES_JSON: &str = include_str!("../data/cities.json");
pub const COASTLINES_JSON: &str = include_str!("../data/coastlines.json");
pub const CONSTELLATIONS_JSON: &str = include_str!("../data/constellations.json");
pub const MILKY_WAY_JSON: &str = include_str!("../data/milky-way.json");
pub const STARS_CATALOG_JSON: &str = include_str!("../data/stars-catalog.json");
pub const STARS_NEARBY_JSON: &str = include_str!("../data/stars-nearby.json");

// ── TOML config ─────────────────────────────────────────────────────
pub const CONFIG_TOML: &str = include_str!("../data/config.toml");
pub const CALENDAR_TOML: &str = include_str!("../data/calendar.toml");
pub const INPUT_TOML: &str = include_str!("../data/input.toml");
pub const PLAYER_TOML: &str = include_str!("../data/player.toml");

// ── GUI theme (RON) ─────────────────────────────────────────────────
pub const THEME_RON: &str = include_str!("../data/gui/theme.ron");

// ── Planet definitions (RON) ────────────────────────────────────────
pub const PLANET_EARTH_RON: &str = include_str!("../data/planets/earth.ron");
pub const PLANET_MARS_RON: &str = include_str!("../data/planets/mars.ron");
pub const PLANET_MOON_RON: &str = include_str!("../data/planets/moon.ron");

// ── Solar system body definitions (RON) ─────────────────────────────
pub const SOLAR_BODY_EARTH_RON: &str = include_str!("../data/solar_system/earth.ron");
pub const SOLAR_BODY_MARS_RON: &str = include_str!("../data/solar_system/mars.ron");
pub const SOLAR_BODY_SUN_RON: &str = include_str!("../data/solar_system/sun.ron");

// ── World data (RON) ───────────────────────────────────────────────
pub const WORLD_SOLAR_SYSTEM_RON: &str = include_str!("../data/world/solar_system.ron");
pub const WORLD_SPAWN_RON: &str = include_str!("../data/world/spawn.ron");
pub const WORLD_PLAYER_RON: &str = include_str!("../data/world/player.ron");

// ── Ship data (RON) ────────────────────────────────────────────────
pub const SHIP_BRIDGE_RON: &str = include_str!("../data/ships/bridge.ron");
pub const SHIP_LAYOUT_MEDIUM_RON: &str = include_str!("../data/ships/layout_medium.ron");
pub const SHIP_REACTOR_RON: &str = include_str!("../data/ships/reactor.ron");
pub const SHIP_STARTER_FLEET_RON: &str = include_str!("../data/ships/starter_fleet.ron");

// ── Quest data (RON) ───────────────────────────────────────────────
pub const QUEST_CONSTRUCTION_RON: &str = include_str!("../data/quests/construction.ron");
pub const QUEST_EXPLORATION_RON: &str = include_str!("../data/quests/exploration.ron");
pub const QUEST_FARMING_RON: &str = include_str!("../data/quests/farming.ron");
pub const QUEST_TUTORIAL_RON: &str = include_str!("../data/quests/tutorial.ron");

// ── Blueprint data (RON) ───────────────────────────────────────────
pub const BLUEPRINT_BASIC_RON: &str = include_str!("../data/blueprints/basic.ron");
pub const BLUEPRINT_CONSTRUCTION_RON: &str = include_str!("../data/blueprints/construction.ron");
pub const BLUEPRINT_HABITAT_RON: &str = include_str!("../data/blueprints/habitat.ron");
pub const BLUEPRINT_MATERIALS_RON: &str = include_str!("../data/blueprints/materials.ron");
pub const BLUEPRINT_OBJECTS_RON: &str = include_str!("../data/blueprints/objects.ron");

// ── Entity templates (RON) ─────────────────────────────────────────
pub const ENTITY_HUMAN_001_RON: &str = include_str!("../data/entities/human/human_001.ron");
pub const ENTITY_PLANT_001_RON: &str = include_str!("../data/entities/plants/plant_001.ron");
pub const ENTITY_TOMATO_RON: &str = include_str!("../data/entities/plants/tomato.ron");
pub const ENTITY_SUBSTRATE_LOAM_RON: &str = include_str!("../data/entities/substrates/loam_basic.ron");
pub const ENTITY_SUBSTRATE_001_RON: &str = include_str!("../data/entities/substrates/substrate_001.ron");

// ── Plots (RON) ────────────────────────────────────────────────────
pub const PLOT_001_RON: &str = include_str!("../data/plots/plot_001.ron");

// ── Resources (RON) ────────────────────────────────────────────────
pub const RESOURCE_FERTILIZER_RON: &str = include_str!("../data/resources/fertilizer_basic.ron");
pub const RESOURCE_WATER_RON: &str = include_str!("../data/resources/water_clean.ron");

// ── i18n / localization (JSON) ──────────────────────────────────────
pub const I18N_EN_JSON: &str = include_str!("../data/i18n/en.json");
pub const I18N_ES_JSON: &str = include_str!("../data/i18n/es.json");
pub const I18N_FR_JSON: &str = include_str!("../data/i18n/fr.json");
pub const I18N_JA_JSON: &str = include_str!("../data/i18n/ja.json");
pub const I18N_ZH_JSON: &str = include_str!("../data/i18n/zh.json");

// ── Language data (JSON) ────────────────────────────────────────────
pub const LANGUAGE_ACRONYMS_JSON: &str = include_str!("../data/language/acronyms.json");
pub const LANGUAGE_DICTIONARY_JSON: &str = include_str!("../data/language/dictionary.json");
pub const LANGUAGE_PARTS_OF_SPEECH_JSON: &str = include_str!("../data/language/parts_of_speech.json");

// ── Lookup helper ───────────────────────────────────────────────────

/// Look up an embedded data string by its relative path (as used by AssetManager).
/// Returns `None` if the path has no embedded fallback.
pub fn get_embedded(path: &str) -> Option<&'static str> {
    // Normalize path separators to forward slashes for matching
    let normalized = path.replace('\\', "/");
    let key = normalized.as_str();

    match key {
        // CSV
        "items.csv" => Some(ITEMS_CSV),
        "recipes.csv" => Some(RECIPES_CSV),
        "materials.csv" => Some(MATERIALS_CSV),
        "components.csv" => Some(COMPONENTS_CSV),
        "plants.csv" => Some(PLANTS_CSV),
        "game.csv" => Some(GAME_CSV),
        "skills/skills.csv" => Some(SKILLS_CSV),
        "chemistry/elements.csv" => Some(ELEMENTS_CSV),
        "chemistry/alloys.csv" => Some(ALLOYS_CSV),
        "chemistry/compounds.csv" => Some(COMPOUNDS_CSV),
        "chemistry/gases.csv" => Some(GASES_CSV),
        "chemistry/toxins.csv" => Some(TOXINS_CSV),
        "asteroids/types.csv" => Some(ASTEROID_TYPES_CSV),

        // JSON
        "glossary.json" => Some(GLOSSARY_JSON),
        "solar_system/bodies.json" => Some(SOLAR_SYSTEM_JSON),
        "solar-system.json" => Some(SOLAR_SYSTEM_LEGACY_JSON),
        "tools/catalog.json" => Some(TOOLS_CATALOG_JSON),
        "cities.json" => Some(CITIES_JSON),
        "coastlines.json" => Some(COASTLINES_JSON),
        "constellations.json" => Some(CONSTELLATIONS_JSON),
        "milky-way.json" => Some(MILKY_WAY_JSON),
        "stars-catalog.json" => Some(STARS_CATALOG_JSON),
        "stars-nearby.json" => Some(STARS_NEARBY_JSON),

        // TOML
        "config.toml" => Some(CONFIG_TOML),
        "calendar.toml" => Some(CALENDAR_TOML),
        "input.toml" => Some(INPUT_TOML),
        "player.toml" => Some(PLAYER_TOML),

        // RON — GUI
        "gui/theme.ron" => Some(THEME_RON),

        // RON — Planets
        "planets/earth.ron" => Some(PLANET_EARTH_RON),
        "planets/mars.ron" => Some(PLANET_MARS_RON),
        "planets/moon.ron" => Some(PLANET_MOON_RON),

        // RON — Solar system bodies
        "solar_system/earth.ron" => Some(SOLAR_BODY_EARTH_RON),
        "solar_system/mars.ron" => Some(SOLAR_BODY_MARS_RON),
        "solar_system/sun.ron" => Some(SOLAR_BODY_SUN_RON),

        // RON — World
        "world/solar_system.ron" => Some(WORLD_SOLAR_SYSTEM_RON),
        "world/spawn.ron" => Some(WORLD_SPAWN_RON),
        "world/player.ron" => Some(WORLD_PLAYER_RON),

        // RON — Ships
        "ships/bridge.ron" => Some(SHIP_BRIDGE_RON),
        "ships/layout_medium.ron" => Some(SHIP_LAYOUT_MEDIUM_RON),
        "ships/reactor.ron" => Some(SHIP_REACTOR_RON),
        "ships/starter_fleet.ron" => Some(SHIP_STARTER_FLEET_RON),

        // RON — Quests
        "quests/construction.ron" => Some(QUEST_CONSTRUCTION_RON),
        "quests/exploration.ron" => Some(QUEST_EXPLORATION_RON),
        "quests/farming.ron" => Some(QUEST_FARMING_RON),
        "quests/tutorial.ron" => Some(QUEST_TUTORIAL_RON),

        // RON — Blueprints
        "blueprints/basic.ron" => Some(BLUEPRINT_BASIC_RON),
        "blueprints/construction.ron" => Some(BLUEPRINT_CONSTRUCTION_RON),
        "blueprints/habitat.ron" => Some(BLUEPRINT_HABITAT_RON),
        "blueprints/materials.ron" => Some(BLUEPRINT_MATERIALS_RON),
        "blueprints/objects.ron" => Some(BLUEPRINT_OBJECTS_RON),

        // RON — Entities
        "entities/human/human_001.ron" => Some(ENTITY_HUMAN_001_RON),
        "entities/plants/plant_001.ron" => Some(ENTITY_PLANT_001_RON),
        "entities/plants/tomato.ron" => Some(ENTITY_TOMATO_RON),
        "entities/substrates/loam_basic.ron" => Some(ENTITY_SUBSTRATE_LOAM_RON),
        "entities/substrates/substrate_001.ron" => Some(ENTITY_SUBSTRATE_001_RON),

        // RON — Plots
        "plots/plot_001.ron" => Some(PLOT_001_RON),

        // RON — Resources
        "resources/fertilizer_basic.ron" => Some(RESOURCE_FERTILIZER_RON),
        "resources/water_clean.ron" => Some(RESOURCE_WATER_RON),

        // JSON — i18n
        "i18n/en.json" => Some(I18N_EN_JSON),
        "i18n/es.json" => Some(I18N_ES_JSON),
        "i18n/fr.json" => Some(I18N_FR_JSON),
        "i18n/ja.json" => Some(I18N_JA_JSON),
        "i18n/zh.json" => Some(I18N_ZH_JSON),

        // JSON — Language
        "language/acronyms.json" => Some(LANGUAGE_ACRONYMS_JSON),
        "language/dictionary.json" => Some(LANGUAGE_DICTIONARY_JSON),
        "language/parts_of_speech.json" => Some(LANGUAGE_PARTS_OF_SPEECH_JSON),

        _ => None,
    }
}
