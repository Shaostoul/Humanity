# Infinite-of-X Principle

> Anything that can exist more than once is a data file, not code.

Everything on HumanityOS is designed for civilization scale. Two people using it today must be the same codebase that handles two billion people using it in twenty years. Ten items in the inventory today must be the same inventory system that handles ten million items. Hard-coding the first of anything forces a refactor when the second arrives. Do it right the first time.

This doc is the rule, the gate, and the audit.

## The rule

If a thing in your code could ever have a sibling, it goes in a file under `data/`, not in a `vec![]` literal. If a string is user-visible, it goes through i18n or the help registry. If a number is a tuning value, it goes in a config.

**Categories of things that must be data-driven:**

- Planets, moons, stars, asteroids (physical objects)
- Items, recipes, ingredients, materials
- Skills, traits, stats, progression curves
- NPCs, factions, dialogue, personalities
- Quests, events, achievements
- Shaders, materials, textures, biomes
- Vehicles, ships, buildings, furniture
- Commands, hotbars, keybindings, shortcuts
- Help topics, tooltips, glossary entries
- Settings categories, form fields
- Themes, color palettes, accent presets
- Marketplace categories, task labels, server roles
- Streaming scenes, studio presets
- Audio clips, music tracks, sound effects

## The pre-ship checklist

Every new feature passes this gate before it ships.

1. **Sibling check.** Could this thing plausibly have a second instance? If yes, data file, not code.
2. **Behavior check.** Any numeric or string value a modder or tuner might want to change? Config file.
3. **UI chrome check.** All colors, spacing, radius, fonts pulled from theme tokens, not literals.
4. **Label check.** User-visible strings routed through i18n (`data/i18n/`) or the help registry (`data/help/topics.json`).
5. **Parity check.** Does native need this too? If yes, port before shipping or open a ticket explicitly scoping the native work.
6. **Checklist check.** Did you update `docs/FEATURES.md` and (if applicable) `docs/STATUS.md`?

Failing any step means you are making a refactor your future self will curse you for.

## Current audit (snapshot at v0.91.5)

Automated scan of `src/` and `web/` for `vec![ X, Y, Z, ... ]` patterns and large static arrays. Fourteen cases identified, ranked by priority.

### High priority

| Where | What | Target file |
|-------|------|-------------|
| `src/gui/mod.rs:840-849` | 8 hardcoded player skills with XP defaults | `data/skills/default_profile.json` |
| `src/gui/pages/resources.rs:22-92` | 6 resource categories with 30+ hardcoded URLs | `data/resources/catalog.json` |
| `src/gui/pages/crafting.rs:11-13` | 12 hardcoded crafting categories | `data/crafting/categories.json` |
| `src/gui/mod.rs:396-405` | 8 studio scene presets | `data/studio/scenes.json` |
| `src/gui/mod.rs:406-411` | 4 default studio sources | `data/studio/sources.json` |
| `src/gui/mod.rs:1048-1058` | 8-planet solar system | `data/solar_system/planets/*.json` (may already exist) |

### Medium priority

| Where | What | Target file |
|-------|------|-------------|
| `src/gui/pages/market.rs:12-15` | 9 market categories | `data/market/categories.json` |
| `src/gui/pages/inventory.rs:13-17` | 6 equipment slots | `data/inventory/equipment_slots.json` |
| `src/gui/pages/studio.rs:20-35` | Streaming platforms, resolutions, FPS, positions | `data/studio/streaming_config.json` |
| `src/gui/pages/bugs.rs:8-9` | Bug severities (4) and categories (6) | `data/bugs/taxonomy.json` |
| `web/chat/chat-ui.js:1652-1720` | 30+ command palette items | `web/data/commands.json` (or `data/commands.json`) |
| `web/shared/settings.js:10-59` | Accent presets, font sizes, theme variants | `data/themes/presets.json` |

### Low priority

| Where | What | Target file |
|-------|------|-------------|
| `src/gui/pages/donate.rs:122-143` | 5 FAQ entries | `data/donate/faq.json` |
| `web/chat/chat-ui.js:70-77` | 6 notification sound presets | `data/sounds/presets.json` |

## Already done (reference patterns)

Study these to understand the target architecture:

- **Items**: `data/items.csv` → `src/ecs/components/inventory.rs` loads via AssetManager.
- **Recipes**: `data/recipes.csv` → crafting system.
- **Plants**: `data/plants.csv` → farming system.
- **Planets**: `data/solar_system/planets/*.ron` → terrain loader.
- **Quests**: `data/quests/*.ron` → quest system.
- **Ships**: `data/ships/*.ron` → ship layout parser.
- **Chemistry**: `data/chemistry/*` → 396 entries (elements, alloys, compounds, gases, toxins).
- **Tools catalog**: `data/tools/catalog.json` → loaded by `load_tools_catalog()` in `src/gui/mod.rs`.
- **Onboarding quests**: `data/onboarding/quests.json` → loaded by `/onboarding` web page.
- **AI onboarding steps**: `data/ai/onboarding.json` → structured flow for AI agents.
- **Theme tokens**: `data/gui/theme.ron` → loaded by `src/gui/theme.rs`.
- **i18n**: `data/i18n/*.json` → 5 language files loaded at runtime.
- **Glossary**: `data/glossary.json` → 150+ term definitions for overlay.

The pattern: **open file, deserialize, iterate. Never hardcode a list.**

## Migration template

When you move a hardcoded list into a data file, follow this template:

1. **Create the data file** under `data/<category>/<name>.{json,ron,csv,toml}`. Pick the format that fits the shape of the data (CSV for flat tables, TOML for small config, RON for Rust-shaped structs, JSON for universal consumption).
2. **Define the schema** as a Rust struct (or TypeScript if web-only) with `#[derive(Deserialize)]`. Put the struct next to the code that uses it.
3. **Replace the hardcoded vec!** with a load function. Graceful fallback: if the file is missing or malformed, log a warning and return a sensible default (often an empty vec), do not panic.
4. **Hot reload** where it makes sense: `AssetManager` + `FileWatcher` already exist. Subscribe to changes, re-parse on file change.
5. **Schema file**: if the data shape is complex, add `schemas/<name>.toml` documenting the fields, valid ranges, and relationships. Modders read this first.
6. **Documentation**: add an entry to `docs/FEATURES.md` under the relevant system so the capability is discoverable.

## Checklist: before opening a PR

Before you commit new code that adds any list-shaped data, ask:

- [ ] Is this a list of things that could grow? → data file.
- [ ] Does the list have user-visible strings? → i18n routing.
- [ ] Does the list have any numeric tuning values? → config file or inline with defaults.
- [ ] Is there a Rust struct for parsing? `#[derive(Deserialize)]` on it.
- [ ] Graceful degradation on missing/malformed data?
- [ ] Schema or comment in the data file explaining the shape?
- [ ] Entry in `docs/FEATURES.md` if this is user-facing?
- [ ] Native AND web consume the same data file where possible?

If any box is unchecked and there isn't a documented reason, the design is not done yet.

## Why this matters

Every refactor is a bug opportunity. Every hardcoded list is a future refactor. Every refactor across a web + native codebase is two bug opportunities. Data-driven from day one is the cheapest way to scale the codebase from its current size toward the infinite user, infinite planet, infinite everything that the mission requires.

Poverty ends through leverage. Leverage comes from building once and reusing, not rewriting.
